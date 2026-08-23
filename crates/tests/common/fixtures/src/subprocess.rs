//! Spawning a long-lived child that carries this installation's subprocess
//! markers, so a test can watch the production code recognise, reclaim, or
//! signal it.
//!
//! The child has to be a binary whose environment the platform will hand back.
//! Linux reads any same-uid `/proc/<pid>/environ`, but macOS withholds the
//! environment of Apple's hardened-runtime binaries — `/bin/sleep` among them,
//! and a copy keeps the signature that hides it. Re-executing the calling
//! crate's own test binary produces an ordinary child on both platforms,
//! matching the MCP servers and agents the supervisor spawns in production.
//!
//! Each caller supplies the full test path of its own helper, which must sleep
//! long enough to outlive the assertions and write [`HELPER_READY_ENV`] once it
//! is running:
//!
//! ```ignore
//! const MARKER_HELPER: &str = "services::process::cleanup_live::marker_helper";
//!
//! #[test]
//! #[ignore = "re-executed as a child process by the tests in this module"]
//! fn marker_helper() {
//!     systemprompt_test_fixtures::subprocess::announce_helper_ready();
//!     std::thread::sleep(std::time::Duration::from_secs(30));
//! }
//! ```

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

pub const HELPER_READY_ENV: &str = "SYSTEMPROMPT_TEST_HELPER_READY";

pub struct Helper {
    command: String,
    ready: PathBuf,
    _ready_dir: TempDir,
}

impl Helper {
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    #[must_use]
    pub fn ready_path(&self) -> &Path {
        &self.ready
    }

    // Waiting on the readiness file is not politeness: between `fork` and
    // `execve` the child still carries the parent's environment, so probing any
    // earlier would confirm an identity the child had not been given yet. The
    // file is written from inside the helper test, so its appearance also
    // proves `--exact <helper_test>` resolved rather than silently matching
    // nothing.
    pub fn await_ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.ready.exists() {
            assert!(
                Instant::now() < deadline,
                "helper process never reached `{}`",
                self.command
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[must_use]
pub fn helper(helper_test: &str) -> Helper {
    let ready_dir = tempfile::tempdir().expect("tempdir");
    let ready = ready_dir.path().join("ready");
    let exe = std::env::current_exe().expect("test binary path");

    Helper {
        command: format!(
            "{} --exact {} --ignored",
            shell_quote(&exe.display().to_string()),
            shell_quote(helper_test)
        ),
        ready,
        _ready_dir: ready_dir,
    }
}

pub struct MarkedChild {
    pub child: Child,
    _helper: Helper,
}

impl MarkedChild {
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for MarkedChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn announce_helper_ready() {
    if let Ok(path) = std::env::var(HELPER_READY_ENV) {
        std::fs::write(path, b"ready").expect("helper readiness file");
    }
}

pub fn spawn_marked_child(helper_test: &str, service_name: &str) -> MarkedChild {
    let helper = helper(helper_test);

    let child = Command::new(std::env::current_exe().expect("test binary path"))
        .args(["--exact", helper_test, "--ignored"])
        .env(HELPER_READY_ENV, helper.ready_path())
        .env(systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV, "1")
        .env(
            systemprompt_models::subprocess::MCP_SERVICE_ID_ENV,
            service_name,
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn helper");

    helper.await_ready();

    MarkedChild {
        child,
        _helper: helper,
    }
}
