use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::install::{InstallError, InstallOptions, install};
use tempfile::TempDir;

struct Dirs {
    home: TempDir,
    config: TempDir,
    data: TempDir,
    state: TempDir,
}

impl Dirs {
    fn new() -> Self {
        Self {
            home: TempDir::new().expect("home"),
            config: TempDir::new().expect("config"),
            data: TempDir::new().expect("data"),
            state: TempDir::new().expect("state"),
        }
    }

    fn org_plugins(&self) -> std::path::PathBuf {
        self.data.path().join("Claude").join("org-plugins")
    }

    fn sentinel(&self) -> std::path::PathBuf {
        self.state
            .path()
            .join("systemprompt-bridge")
            .join("metadata")
            .join("version.json")
    }

    // Pins the system org-plugins root into the sandbox so an install can
    // never provision the host's real one. It points at the same path the
    // assertions read: macOS takes the system scope unconditionally, so an
    // unwritable decoy here fails the install outright rather than falling
    // back to user scope the way Linux does. Scope selection itself is
    // covered per-platform in the `paths` suite; these tests are about
    // SUDO_USER handling and idempotence.
    fn system_org_plugins(&self) -> String {
        self.org_plugins().display().to_string()
    }

    fn run<R>(&self, sudo_user: Option<&str>, f: impl FnOnce() -> R) -> R {
        let vars: Vec<(&str, Option<String>)> = vec![
            ("HOME", Some(self.home.path().display().to_string())),
            (
                "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
                Some(self.system_org_plugins()),
            ),
            (
                "XDG_CONFIG_HOME",
                Some(self.config.path().display().to_string()),
            ),
            (
                "XDG_DATA_HOME",
                Some(self.data.path().display().to_string()),
            ),
            (
                "XDG_STATE_HOME",
                Some(self.state.path().display().to_string()),
            ),
            (
                "XDG_CACHE_HOME",
                Some(self.home.path().display().to_string()),
            ),
            ("SUDO_USER", sudo_user.map(str::to_owned)),
        ];
        temp_env::with_vars(vars, f)
    }
}

fn options() -> InstallOptions {
    InstallOptions::builder().build()
}

#[cfg(unix)]
#[test]
fn a_root_owned_sudo_user_marker_is_ignored() {
    let dirs = Dirs::new();
    dirs.run(Some("root"), || {
        let _installed = install(&options(), &bridge())
            .expect("install succeeds under a user-scoped org-plugins root");
    });
    assert!(dirs.sentinel().is_file(), "sentinel written");
    assert!(dirs.org_plugins().is_dir(), "org-plugins root created");
}

#[cfg(unix)]
#[test]
fn an_empty_sudo_user_marker_is_ignored() {
    let dirs = Dirs::new();
    dirs.run(Some(""), || {
        let _installed = install(&options(), &bridge()).expect("install succeeds");
    });
    assert!(dirs.sentinel().is_file());
}

#[cfg(unix)]
#[test]
fn an_unresolvable_sudo_user_fails_the_install_before_the_sentinel() {
    let dirs = Dirs::new();
    let err = dirs.run(Some("no-such-user-987654"), || {
        install(&options(), &bridge()).expect_err("an unresolvable SUDO_USER cannot be chowned to")
    });
    let InstallError::Partial { completed, source } = err else {
        panic!("directory bootstrap runs first and reports partial progress, got {err:?}");
    };
    assert!(
        completed.is_empty(),
        "nothing is recorded as done before ownership is verified, got {completed:?}"
    );
    let InstallError::Bootstrap(message) = *source else {
        panic!("the failure is the bootstrap step, got {source:?}");
    };
    assert!(
        message.contains("no-such-user-987654"),
        "the error names the user that could not be resolved: {message}"
    );
    assert!(
        !dirs.sentinel().exists(),
        "an install whose ownership could not be restored must not look complete"
    );
}

#[cfg(unix)]
#[test]
fn a_resolvable_sudo_user_still_completes_the_install() {
    let dirs = Dirs::new();
    let me = std::env::var("USER").unwrap_or_else(|_| "root".to_owned());
    dirs.run(Some(&me), || {
        let _installed = install(&options(), &bridge()).expect("install succeeds");
    });
    assert!(dirs.sentinel().is_file());
    assert!(dirs.org_plugins().is_dir());
}

#[test]
fn install_is_idempotent() {
    let dirs = Dirs::new();
    let (first, second) = dirs.run(None, || {
        let first = install(&options(), &bridge()).expect("first install");
        let second = install(&options(), &bridge()).expect("second install");
        (first.location.path.clone(), second.location.path.clone())
    });
    assert_eq!(first, second, "both installs resolve the same location");
    assert!(dirs.sentinel().is_file());
}

fn bridge() -> std::sync::Arc<BridgeContext> {
    BridgeContext::start(ProxyMode::Attach).expect("runtime builds")
}

#[cfg(unix)]
#[test]
fn a_sudo_user_install_leaves_the_tree_owned_by_that_user_root_and_children_alike() {
    use std::os::unix::fs::MetadataExt;

    let me = String::from_utf8(
        std::process::Command::new("/usr/bin/id")
            .arg("-un")
            .output()
            .expect("id -un")
            .stdout,
    )
    .expect("utf-8 user name");
    let me = me.trim().to_owned();
    if me == "root" {
        panic!("this test needs a non-root user so SUDO_USER is honoured rather than ignored");
    }

    let dirs = Dirs::new();
    let root = dirs.org_plugins();
    dirs.run(Some(&me), || {
        install(&options(), &bridge()).expect("install completes for a resolvable SUDO_USER");
        // Why: ownership is verified on the root *and* a sampled child; an
        // empty tree would pass that check without ever reading a child.
        std::fs::create_dir_all(root.join("an-existing-plugin")).expect("seed a child");
        install(&options(), &bridge()).expect("a second install re-verifies the populated tree");
    });

    let expected = std::fs::metadata(dirs.data.path()).expect("data dir metadata");
    let actual = std::fs::metadata(&root).expect("org-plugins metadata");
    assert_eq!(
        (actual.uid(), actual.gid()),
        (expected.uid(), expected.gid()),
        "the provisioned root belongs to the invoking user, not to root"
    );

    let child = root.join("an-existing-plugin");
    let child_meta = std::fs::metadata(&child).expect("child metadata");
    assert_eq!(
        (child_meta.uid(), child_meta.gid()),
        (expected.uid(), expected.gid()),
        "ownership is verified recursively, so {} must match too",
        child.display()
    );
}
