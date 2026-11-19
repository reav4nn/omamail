use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub(crate) struct TempDir(PathBuf);

impl TempDir {
    pub(crate) fn new() -> Self {
        let template = std::env::temp_dir().join("omamail-test-XXXXXX");
        let mut bytes = template.as_os_str().as_bytes().to_vec();
        assert!(!bytes.contains(&0));
        bytes.push(0);
        // SAFETY: mkdtemp takes a mutable, NUL-terminated template ending in XXXXXX.
        // It replaces the final six bytes and creates the directory with mode 0700.
        let result = unsafe { libc::mkdtemp(bytes.as_mut_ptr().cast()) };
        assert!(!result.is_null(), "mkdtemp: {}", std::io::Error::last_os_error());
        Self(PathBuf::from(OsStr::from_bytes(&bytes[..bytes.len() - 1])))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn attached_mail(part_headers: &str, encoded_body: &str) -> String {
    format!(
        "Subject: Outer mail\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=outer\r\n\r\n\
--outer\r\nContent-Type: text/plain\r\n\r\nSee attachment\r\n\
--outer\r\nContent-Type: message/rfc822\r\n{part_headers}\r\n{encoded_body}\r\n--outer--\r\n"
    )
}
