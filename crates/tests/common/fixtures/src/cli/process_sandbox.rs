//! A process sandbox for tests that drive commands which shell out: an owned
//! project root as the working directory and a `PATH` holding only the stub
//! executables a test writes. Dropping it restores both.

use std::ffi::OsString;
use std::path::PathBuf;

pub struct Sandbox {
    pub root: tempfile::TempDir,
    previous: PathBuf,
    path: Option<OsString>,
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Sandbox {
    pub fn new() -> Self {
        let root = crate::canonical_tempdir();
        std::fs::create_dir_all(root.path().join(".systemprompt/docker")).unwrap();
        std::fs::create_dir(root.path().join("bin")).unwrap();
        std::fs::write(root.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        let previous = std::env::current_dir().unwrap();
        let path = std::env::var_os("PATH");
        std::env::set_current_dir(root.path()).unwrap();
        // SAFETY: nextest isolates each test in a process; child commands use only this
        // sandbox.
        unsafe {
            std::env::set_var("PATH", root.path().join("bin"));
        }
        Self {
            root,
            previous,
            path,
        }
    }
    pub fn executable(&self, name: &str, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.root.path().join("bin").join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    pub fn calls(&self) -> String {
        std::fs::read_to_string(self.root.path().join("calls")).unwrap()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.previous).unwrap();
        // SAFETY: restores the process-local environment installed by this test.
        unsafe {
            match &self.path {
                Some(path) => std::env::set_var("PATH", path),
                None => std::env::remove_var("PATH"),
            }
        }
    }
}
