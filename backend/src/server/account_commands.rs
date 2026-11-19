use super::*;
use crate::accounts::Entry;
use crate::secret;

impl Server {
    pub(super) fn active_id(&self) -> &str {
        self.accounts.active.as_deref().unwrap_or("")
    }
    pub(super) fn token(&self) -> String {
        format!("{}:{}", self.boot, self.epoch)
    }
    pub(super) fn status(&self) -> Value {
        json!({"configured": self.session.is_some() || (cfg!(feature="demo") && self.accounts.selected().is_some()),
            "email": self.accounts.selected().map(|e| &e.account.email), "account": self.active_id(),
            "accounts": self.accounts.accounts.iter().map(|e| json!({"id":e.id, "email":e.account.email})).collect::<Vec<_>>(),
            "session": self.token(), "version":VERSION, "error":self.credential_error})
    }
    pub(super) fn restore(&mut self) {
        self.credential_error.clear();
        if let Some(e) = self.accounts.selected() {
            match secret::lookup(&e.account.email) {
                Ok(Some(p)) => self.start_session(e.account.clone(), p),
                Ok(None) => {
                    self.credential_error =
                        "No app password in the keyring. Sign in again or choose another account."
                            .into()
                }
                Err(e) => self.credential_error = e,
            }
        }
    }
    pub(super) fn change_account(&mut self, cmd: &str, req: &Value) -> Reply {
        let mut next = self.accounts.clone();
        if cmd == "remove" {
            let id = str_arg(req, "account")?;
            let entry = next
                .by_id(id)
                .ok_or_else(|| fail("Account no longer exists"))?
                .clone();
            // A slow render may still be writing this account's images.
            for (_, job) in self.images.extract_if(.., |(owner, _)| owner == id) {
                job.join()
                    .map_err(|_| fail("Image worker stopped unexpectedly"))?;
            }
            next.remove(id);
            let changed = next.active != self.accounts.active;
            let lock = accounts::lock().map_err(fail_io)?;
            self.accounts
                .commit(&next, || {
                    let email = &entry.account.email;
                    let previous = secret::lookup(email)?;
                    if previous.is_some() {
                        secret::clear(email)?;
                    }
                    if let Err(e) = accounts::clear_local(id) {
                        if let Some(p) = previous {
                            secret::store(email, &p).map_err(|r| format!("{e}; {r}"))?;
                        }
                        return Err(format!("Could not remove local mail data: {e}"));
                    }
                    Ok(())
                })
                .map_err(fail)?;
            self.accounts = next;
            drop(lock);
            if changed {
                self.end_session();
                self.epoch += 1;
                self.restore();
            }
        } else {
            let (entry, password) = if cmd == "select" {
                let e = next
                    .by_id(str_arg(req, "account")?)
                    .ok_or_else(|| fail("Account no longer exists"))?
                    .clone();
                let p = secret::lookup(&e.account.email)
                    .map_err(fail)?
                    .ok_or_else(|| fail("No app password. Sign in again for this account."))?;
                (e, p)
            } else {
                let email = str_arg(req, "email")?.trim();
                if email.parse::<lettre::Address>().is_err() || email.chars().any(char::is_control)
                {
                    return Err(fail("Enter your full Gmail address"));
                }
                let p: String = str_arg(req, "password")?.split_whitespace().collect();
                if p.is_empty() {
                    return Err(fail("Enter the app password"));
                }
                let e = match next.by_email(email) {
                    Some(e) => e.clone(),
                    None => {
                        let e = Entry {
                            id: accounts::nonce().map_err(fail_io)?,
                            account: Account::gmail(email),
                        };
                        next.accounts.push(e.clone());
                        e
                    }
                };
                (e, p)
            };
            let mut probe = Mailbox::new(entry.account.clone(), password.clone());
            probe.connect()?;
            probe.close();
            next.active = Some(entry.id.clone());
            let lock = accounts::lock().map_err(fail_io)?;
            self.accounts
                .commit(&next, || {
                    if cmd == "login" {
                        secret::store(&entry.account.email, &password)
                    } else {
                        Ok(())
                    }
                })
                .map_err(fail)?;
            self.accounts = next;
            drop(lock);
            self.end_session();
            self.epoch += 1;
            self.credential_error.clear();
            self.start_session(entry.account, password);
        }
        if cmd != "select" {
            watcher(if self.accounts.accounts.is_empty() {
                "stop"
            } else {
                "restart"
            });
        }
        Ok(self.status())
    }
}
