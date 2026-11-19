//! Exercise the real stdio/CLI boundary in a disposable desktop environment.
//! No Gmail, network server, user config, clipboard, keyring or bus is used.
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Stdio};

#[cfg(not(feature = "demo"))]
const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
#[cfg(not(feature = "demo"))]
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
#[cfg(not(feature = "demo"))]
const HOME: &str = "Home@example.test";
#[cfg(not(feature = "demo"))]
const WORK: &str = "Work@example.test";
struct Desktop(PathBuf);
impl Desktop {
    fn new() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let dir = std::env::temp_dir().join(format!("omamail-session-test-{}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let me = Self(dir);
        for (name, script) in [
            (
                "secret-tool",
                "case $1 in lookup) exit 1;; clear) exit 0;; *) exit 2;; esac",
            ),
            ("systemctl", "exit 0"),
            ("omarchy-shell", "printf '%s' \"$4\" > \"$HOME/opened\""),
            (
                "wl-copy",
                "[ \"$1\" = --sensitive ] || exit 2; IFS= read -r value; printf '%s' \"$value\" > \"$HOME/copied\"",
            ),
        ] {
            me.script(name, script);
        }
        me
    }
    fn script(&self, name: &str, body: &str) {
        let path = self.0.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_omamail"));
        cmd.args(args)
            .env("HOME", &self.0)
            .env("PATH", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("XDG_STATE_HOME", self.0.join("state"))
            .env("XDG_CACHE_HOME", self.0.join("cache"))
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                "unix:path=/nonexistent/omamail-test",
            );
        cmd
    }
    #[cfg(not(feature = "demo"))]
    fn write(&self, path: &str, data: Value) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data.to_string()).unwrap();
    }
    fn serve(&self) -> Panel {
        let mut p = self
            .command(&["serve"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let out = BufReader::new(p.stdout.take().unwrap());
        Panel { p, out, seq: 0 }
    }
    fn click(&self, args: &[&str]) -> bool {
        self.command(args).status().unwrap().success()
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Panel {
    p: Child,
    out: BufReader<ChildStdout>,
    seq: u64,
}
impl Panel {
    fn request(&mut self, mut req: Value) -> Value {
        self.seq += 1;
        req["id"] = json!(self.seq);
        writeln!(self.p.stdin.as_mut().unwrap(), "{req}").unwrap();
        loop {
            let mut line = String::new();
            assert_ne!(self.out.read_line(&mut line).unwrap(), 0);
            let reply: Value = serde_json::from_str(&line).unwrap();
            if reply["id"] == self.seq {
                return reply;
            }
        }
    }
    fn ok(&mut self, req: Value) -> Value {
        let r = self.request(req);
        assert_eq!(r["ok"], true, "{r}");
        r["data"].clone()
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        self.p.stdin.take();
        let _ = self.p.wait();
    }
}

#[cfg(not(feature = "demo"))]
#[test]
fn overlapping_uids_missing_credentials_restart_removal_and_notification_clicks() {
    let d = Desktop::new();
    // Loopback (port 1) even if a credential test double accidentally succeeds.
    let entries: Vec<_> = [(A, HOME), (B, WORK)]
        .iter()
        .map(|(id, email)| json!({"id":id,"email":email,"imap_host":"127.0.0.1","imap_port":1}))
        .collect();
    d.write(
        "config/omamail/config.json",
        json!({"accounts":entries,"active":A}),
    );
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    for (id, email, code) in [(A, HOME, "111111"), (B, WORK, "222222")] {
        d.write(&format!("state/omamail/accounts/{id}/watch.json"),json!({"email":email,"last_uid":42,"uidvalidity":7,"codes":[{"uid":42,"code":code,"saved_at":now}]}));
        d.write(&format!("state/omamail/accounts/{id}/list.json"),json!({"v":1,"email":email,"folders":{"INBOX":{"messages":[],"uidvalidity":7,"total":1,"used":1}}}));
        d.write(
            &format!("cache/omamail/accounts/{id}/inline/img"),
            json!("image"),
        );
        assert!(d.click(&["copy", id, "7", "42"]));
        assert_eq!(fs::read_to_string(d.0.join("copied")).unwrap(), code);
        assert!(d.click(&["open", id, "7", "42"]));
        let payload: Value =
            serde_json::from_slice(&fs::read(d.0.join("opened")).unwrap()).unwrap();
        assert_eq!(
            payload,
            json!({"account":id,"folder":"INBOX","uid":42,"uidvalidity":7})
        );
    }
    let mut p = d.serve();
    let home = p.ok(json!({"cmd":"status"}));
    assert_eq!(home["account"], A);
    assert_eq!(home["configured"], false);
    assert!(!home["error"].as_str().unwrap().is_empty());
    assert_eq!(
        p.request(json!({"cmd":"select","account":B,"session":home["session"]}))["ok"],
        false
    );
    assert_eq!(p.ok(json!({"cmd":"status"})), home);
    // The other account's password must not be cleared. On failure nothing is lost.
    d.script(
        "secret-tool",
        "case $1 in lookup) printf test;; clear) exit 2;; esac",
    );
    assert_eq!(
        p.request(json!({"cmd":"remove","account":A,"session":home["session"]}))["ok"],
        false
    );
    assert!(d.click(&["copy", A, "7", "42"]));
    d.script("secret-tool", "case $1 in lookup) [ \"$5\" = Home@example.test ] || exit 1; printf synthetic;; clear) printf '%s' \"$5\" > \"$HOME/cleared\";; esac");
    let work = p.ok(json!({"cmd":"remove","account":A,"session":home["session"]}));
    assert_eq!(work["account"], B);
    assert_ne!(home["session"], work["session"]);
    assert_eq!(fs::read_to_string(d.0.join("cleared")).unwrap(), HOME);
    for dir in ["state", "cache"] {
        let root = d.0.join(dir).join("omamail/accounts");
        assert!(!root.join(A).exists() && root.join(B).exists());
    }
    for cmd in ["copy", "open"] {
        assert!(!d.click(&[cmd, A, "7", "42"]));
        assert!(!d.click(&[cmd, B, "6", "42"]));
        assert!(!d.click(&[cmd, "42"]));
    }
    assert!(d.click(&["copy", B, "7", "42"]));
    assert_eq!(fs::read_to_string(d.0.join("copied")).unwrap(), "222222");
    drop(p);
    let mut p = d.serve();
    let resumed = p.ok(json!({"cmd":"status"}));
    assert_eq!(resumed["account"], B);
    assert_ne!(resumed["session"], work["session"]);
    assert_eq!(
        p.request(json!({"cmd":"remove","account":B,"session":work["session"]}))["ok"],
        false
    );
    let empty = p.ok(json!({"cmd":"remove","account":B,"session":resumed["session"]}));
    assert_eq!(empty["accounts"], json!([]));
    assert_eq!(empty["configured"], false);
}

#[cfg(feature = "demo")]
#[test]
fn demo_select_add_fail_reauth_remove_and_reject_stale_commands() {
    let d = Desktop::new();
    // A real configuration read would fail, not silently look empty.
    fs::write(d.0.join("config"), b"sentinel").unwrap();
    for name in ["secret-tool", "systemctl", "wl-copy", "omarchy-shell"] {
        d.script(name, "printf used > \"$HOME/used-desktop\"; exit 2");
    }
    let mut p = d.serve();
    let first = p.ok(json!({"cmd":"status"}));
    let page = |s: &Value| json!({"cmd":"list","session":s["session"],"folder":"INBOX"});
    let a = p.ok(page(&first));
    let second = p.ok(
        json!({"cmd":"select","session":first["session"],"account":first["accounts"][1]["id"]}),
    );
    let b = p.ok(page(&second));
    assert_eq!(a["uidvalidity"], b["uidvalidity"]);
    assert_eq!(a["messages"][0]["uid"], b["messages"][0]["uid"]);
    assert_ne!(a["messages"][0]["subject"], b["messages"][0]["subject"]);
    for cmd in [
        "cached",
        "list",
        "open",
        "flag",
        "move",
        "undo",
        "send",
        "attachment",
        "draft",
        "remove",
    ] {
        let r = p.request(json!({"cmd":cmd,"session":first["session"],"uid":42,"uidvalidity":1}));
        assert_eq!(r["error"], "This account session has ended");
    }
    let login = |s: &Value, email: &str, password: &str| json!({"cmd":"login","email":email,"password":password,"session":s["session"]});
    assert_eq!(
        p.request(login(&second, "new@example.test", "invalid"))["ok"],
        false
    );
    assert_eq!(p.ok(json!({"cmd":"status"})), second);
    let new = p.ok(login(&second, "New@example.test", "demo"));
    let again = p.ok(login(&new, "NEW@example.test", "demo"));
    assert_eq!(again["accounts"].as_array().unwrap().len(), 3);
    assert_eq!(again["account"], new["account"]);
    let left = p.ok(json!({"cmd":"remove","account":new["account"],"session":again["session"]}));
    assert_eq!(left["accounts"].as_array().unwrap().len(), 2);
    for cmd in ["watch", "copy", "open", "update-check"] {
        assert!(!d.click(&[cmd]));
    }
    assert!(!d.0.join("used-desktop").exists());
    assert!(!d.0.join("state").exists() && !d.0.join("cache").exists());
    assert_eq!(fs::read(d.0.join("config")).unwrap(), b"sentinel");
}
