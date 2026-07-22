use systemprompt_bridge::install::{InstallOptions, install};
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

    // Pins the system org-plugins root to an unwritable path inside the
    // sandbox so install exercises the user-scope path even on hosts where
    // the real system parent is writable (CI runners).
    fn system_org_plugins(&self) -> String {
        let root = self.data.path().join("system-root");
        std::fs::create_dir_all(&root).expect("system root");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o555))
                .expect("read-only system root");
        }
        root.join("Claude")
            .join("org-plugins")
            .display()
            .to_string()
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
        install(&options()).expect("install succeeds under a user-scoped org-plugins root");
    });
    assert!(dirs.sentinel().is_file(), "sentinel written");
    assert!(dirs.org_plugins().is_dir(), "org-plugins root created");
}

#[cfg(unix)]
#[test]
fn an_empty_sudo_user_marker_is_ignored() {
    let dirs = Dirs::new();
    dirs.run(Some(""), || {
        install(&options()).expect("install succeeds");
    });
    assert!(dirs.sentinel().is_file());
}

#[cfg(unix)]
#[test]
fn an_unresolvable_sudo_user_does_not_abort_the_install() {
    let dirs = Dirs::new();
    dirs.run(Some("no-such-user-987654"), || {
        install(&options()).expect("ownership fixups are best-effort");
    });
    assert!(
        dirs.sentinel().is_file(),
        "a failed SUDO_USER lookup must not fail the install"
    );
}

#[cfg(unix)]
#[test]
fn a_resolvable_sudo_user_still_completes_the_install() {
    let dirs = Dirs::new();
    let me = std::env::var("USER").unwrap_or_else(|_| "root".to_owned());
    dirs.run(Some(&me), || {
        install(&options()).expect("install succeeds");
    });
    assert!(dirs.sentinel().is_file());
    assert!(dirs.org_plugins().is_dir());
}

#[test]
fn install_is_idempotent() {
    let dirs = Dirs::new();
    let (first, second) = dirs.run(None, || {
        let first = install(&options()).expect("first install");
        let second = install(&options()).expect("second install");
        (first.location.path.clone(), second.location.path.clone())
    });
    assert_eq!(first, second, "both installs resolve the same location");
    assert!(dirs.sentinel().is_file());
}
