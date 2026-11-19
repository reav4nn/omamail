//! Durable account identities are independent of the email address: even a
//! removal followed by re-adding the same address invalidates old actions.
use crate::config::{self, Account};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    #[serde(flatten)]
    pub account: Account,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Accounts {
    pub accounts: Vec<Entry>,
    pub active: Option<String>,
}

pub fn nonce() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(format!("{:032x}", u128::from_be_bytes(bytes)))
}

pub fn state(id: &str) -> PathBuf {
    config::state_dir().join("accounts").join(id)
}

fn lock_file(name: &str) -> io::Result<File> {
    let dir = config::config_dir();
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)?;
    File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(dir.join(name))
}

/// The panel owns one serial worker. Never allow a second panel to keep a
/// stale writable copy of its session after this one removes an account.
pub fn panel_lock() -> io::Result<File> {
    let f = lock_file("panel.lock")?;
    f.try_lock().map_err(io::Error::other)?;
    Ok(f)
}

/// Serializes config/keyring transactions with watcher writes and clicks.
pub fn lock() -> io::Result<File> {
    let f = lock_file("accounts.lock")?;
    f.lock()?;
    Ok(f)
}

impl Accounts {
    pub fn by_id(&self, id: &str) -> Option<&Entry> {
        self.accounts.iter().find(|a| a.id == id)
    }
    pub fn by_email(&self, email: &str) -> Option<&Entry> {
        self.accounts
            .iter()
            .find(|a| a.account.email.eq_ignore_ascii_case(email.trim()))
    }
    pub fn selected(&self) -> Option<&Entry> {
        self.active.as_deref().and_then(|id| self.by_id(id))
    }
    pub fn remove(&mut self, id: &str) {
        self.accounts.retain(|a| a.id != id);
        if self.active.as_deref() == Some(id) {
            self.active = self.accounts.first().map(|a| a.id.clone());
        }
    }
    /// Caller holds accounts.lock. All readers use the same migration code.
    pub fn load() -> io::Result<Self> {
        Self::load_at(
            &config::config_dir().join("config.json"),
            &config::state_dir(),
        )
    }
    fn load_at(path: &Path, state_dir: &Path) -> io::Result<Self> {
        let text = match fs::read(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            r => r?,
        };
        let value: serde_json::Value = serde_json::from_slice(&text)?;
        if value.get("accounts").is_some() {
            let saved: Self = serde_json::from_value(value)?;
            for (i, e) in saved.accounts.iter().enumerate() {
                if e.id.len() != 32
                    || !e.id.bytes().all(|b| b.is_ascii_hexdigit())
                    || saved.accounts[..i].iter().any(|o| {
                        o.id == e.id || o.account.email.eq_ignore_ascii_case(&e.account.email)
                    })
                {
                    return Err(io::Error::other("Invalid or duplicate account identity"));
                }
            }
            if saved.active.is_some() == saved.accounts.is_empty()
                || (saved.active.is_some() && saved.selected().is_none())
            {
                return Err(io::Error::other("Invalid active account"));
            }
            return Ok(saved);
        }
        let account: Account = serde_json::from_value(value)?;
        // Deterministic only for the legacy format, so interrupted migration
        // resumes in the same directory. Preserve email case (the keyring key).
        use sha2::{Digest, Sha256};
        let id = format!("{:x}", Sha256::digest(account.email.as_bytes()))[..32].to_string();
        for name in ["watch.json", "list.json"] {
            let old = state_dir.join(name);
            let bytes = match fs::read(&old) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                r => r?,
            };
            if serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .is_some_and(|v| v["email"].as_str() == Some(&account.email))
            {
                config::write_private(&state_dir.join("accounts").join(&id).join(name), &bytes)?;
                fs::remove_file(old)?;
            }
        }
        let migrated = Self {
            active: Some(id.clone()),
            accounts: vec![Entry { id, account }],
        };
        migrated.save_at(path)?;
        Ok(migrated)
    }
    fn save_at(&self, path: &Path) -> io::Result<()> {
        config::write_private(path, &serde_json::to_vec_pretty(self)?)
    }
    /// Settings are atomically replaced before touching the keyring. A disk
    /// failure therefore never changes a password. Readers are locked out
    /// until a failed keyring operation has been rolled back.
    pub fn commit(
        &self,
        next: &Self,
        credential: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        self.commit_at(next, &config::config_dir().join("config.json"), credential)
    }
    fn commit_at(
        &self,
        next: &Self,
        path: &Path,
        credential: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        next.save_at(path)
            .map_err(|e| format!("Could not save accounts: {e}"))?;
        if let Err(e) = credential() {
            self.save_at(path)
                .map_err(|r| format!("{e}; could not restore account settings: {r}"))?;
            return Err(e);
        }
        Ok(())
    }
}

pub fn clear_local(id: &str) -> io::Result<()> {
    for dir in [state(id), config::cache_dir().join("accounts").join(id)] {
        match fs::remove_dir_all(dir) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            result => result?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn migrates_only_verified_state_preserving_hosts_and_keyring_spelling() {
        let dir = TempDir::new();
        let path = dir.path().join("config.json");
        fs::write(&path, br#"{"email":"Home@example.test","imap_host":"old.imap","imap_port":1993,"smtp_host":"old.smtp","smtp_port":1465}"#).unwrap();
        for (file, email) in [
            ("watch.json", "Home@example.test"),
            ("list.json", "other@example.test"),
        ] {
            fs::write(
                dir.path().join(file),
                serde_json::json!({"email":email}).to_string(),
            )
            .unwrap();
        }
        let migrated = Accounts::load_at(&path, dir.path()).unwrap();
        let home = migrated.by_email("HOME@example.test").unwrap();
        assert_eq!(home.account.email, "Home@example.test");
        assert_eq!(
            (&*home.account.imap_host, home.account.imap_port),
            ("old.imap", 1993)
        );
        assert_eq!(
            (&*home.account.smtp_host, home.account.smtp_port),
            ("old.smtp", 1465)
        );
        let scoped = dir.path().join("accounts").join(&home.id);
        assert!(scoped.join("watch.json").exists() && !dir.path().join("watch.json").exists());
        assert!(!scoped.join("list.json").exists());
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn failed_credential_or_settings_write_does_not_switch_accounts() {
        let dir = TempDir::new();
        let path = dir.path().join("config.json");
        let mut saved = Accounts::default();
        for email in ["a@example.test", "b@example.test"] {
            saved.accounts.push(Entry {
                id: nonce().unwrap(),
                account: Account::gmail(email),
            });
        }
        saved.active = Some(saved.accounts[0].id.clone());
        let mut next = saved.clone();
        next.active = Some(saved.accounts[1].id.clone());
        saved.save_at(&path).unwrap();
        assert!(
            saved
                .commit_at(&next, &path, || Err("keyring locked".into()))
                .is_err()
        );
        assert_eq!(
            Accounts::load_at(&path, dir.path()).unwrap().active,
            saved.active
        );
        assert!(
            saved
                .commit_at(&next, dir.path(), || panic!("must not write keyring"))
                .is_err()
        );
        saved.commit_at(&next, &path, || Ok(())).unwrap();
        let mut restarted = Accounts::load_at(&path, dir.path()).unwrap();
        assert_eq!(restarted.active, next.active);
        restarted.remove(&saved.accounts[1].id);
        assert_eq!(restarted.active, saved.active);
        restarted.remove(&saved.accounts[0].id);
        assert!(restarted.selected().is_none());
    }
}
