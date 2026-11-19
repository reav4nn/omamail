use super::*;
use crate::accounts::Entry;

impl Server {
    pub(super) fn init_demo(&mut self) {
        for (id, email) in [
            ("00000000000000000000000000000001", "alex@example.com"),
            ("00000000000000000000000000000002", "work@example.com"),
        ] {
            self.accounts.accounts.push(Entry {
                id: id.into(),
                account: Account::gmail(email),
            });
        }
        self.accounts.active = Some(self.accounts.accounts[0].id.clone());
    }

    pub(super) fn demo(&mut self, req: &Value, cmd: &str) -> Reply {
        match cmd {
            "select" | "remove" => {
                let id = str_arg(req, "account")?;
                if self.accounts.by_id(id).is_none() {
                    return Err(fail("Account no longer exists"));
                }
                if cmd == "select" {
                    self.accounts.active = Some(id.into());
                    self.epoch += 1;
                } else {
                    if self.active_id() == id {
                        self.epoch += 1;
                    }
                    self.accounts.remove(id);
                }
            }
            "login" => {
                let email = str_arg(req, "email")?.trim();
                if email.parse::<lettre::Address>().is_err() || str_arg(req, "password")? != "demo"
                {
                    return Err(fail("Use a synthetic address and the demo password: demo"));
                }
                if self.accounts.by_email(email).is_none() {
                    self.accounts.accounts.push(Entry {
                        id: accounts::nonce().map_err(fail_io)?,
                        account: Account::gmail(email),
                    });
                }
                self.accounts.active = self.accounts.by_email(email).map(|e| e.id.clone());
                self.epoch += 1;
            }
            _ => {
                let e = self
                    .accounts
                    .selected()
                    .ok_or_else(|| fail("Sign in first"))?;
                return crate::demo::dispatch(req, &e.account.email).map_err(fail);
            }
        }
        Ok(self.status())
    }
}
