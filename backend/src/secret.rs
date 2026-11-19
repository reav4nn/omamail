//! App password storage in the desktop keyring (libsecret via `secret-tool`).
//!
//! The password only ever travels over pipes, never argv, so it never shows
//! up in /proc/<pid>/cmdline.

use std::io::Write;
use std::process::{Command, Stdio};

const ATTRS: [&str; 3] = ["application", "omamail", "account"];

pub fn store(account: &str, password: &str) -> Result<(), String> {
    let mut child = Command::new("secret-tool")
        .arg("store")
        .arg(format!("--label=Omamail ({account})"))
        .args(ATTRS)
        .arg(account)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("secret-tool is not available: {e}"))?;
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(password.as_bytes())
        .map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err("Could not save the password to the desktop keyring".into())
    }
}

pub fn lookup(account: &str) -> Result<Option<String>, String> {
    let out = Command::new("secret-tool")
        .arg("lookup")
        .args(ATTRS)
        .arg(account)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Could not read the keyring: {e}"))?;
    if out.status.code() == Some(1) && out.stderr.is_empty() {
        return Ok(None);
    }
    if !out.status.success() {
        return Err("Could not read the desktop keyring".into());
    }
    let password = String::from_utf8(out.stdout).map_err(|_| "Invalid password in keyring")?;
    Ok((!password.is_empty()).then_some(password))
}

pub fn clear(account: &str) -> Result<(), String> {
    let status = Command::new("secret-tool")
        .arg("clear")
        .args(ATTRS)
        .arg(account)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Could not remove password: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("Could not remove password from the keyring".into())
    }
}
