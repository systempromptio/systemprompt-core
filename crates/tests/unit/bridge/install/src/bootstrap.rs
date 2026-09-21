use systemprompt_bridge::context::{BridgeContext, ProxyMode};
#[cfg(unix)]
use systemprompt_bridge::install::InstallError;
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

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    fn run_with_path<R>(&self, path: &std::path::Path, f: impl FnOnce() -> R) -> R {
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
            ("PATH", Some(path.display().to_string())),
            ("SUDO_USER", None),
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
    let InstallError::Bootstrap(source) = *source else {
        panic!("the failure is the bootstrap step, got {source:?}");
    };
    let message = source.to_string();
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
        let _first =
            install(&options(), &bridge()).expect("install completes for a resolvable SUDO_USER");
        // Why: ownership is verified on the root *and* a sampled child; an
        // empty tree would pass that check without ever reading a child.
        std::fs::create_dir_all(root.join("an-existing-plugin")).expect("seed a child");
        let _second = install(&options(), &bridge())
            .expect("a second install re-verifies the populated tree");
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

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn refused_schedule_activation_reports_partial_install_and_a_retry_finishes_it() {
    use std::os::unix::fs::PermissionsExt as _;
    use systemprompt_bridge::install::InstallStep;

    let dirs = Dirs::new();
    let tools = TempDir::new().expect("tool directory");
    let systemctl = tools.path().join("systemctl");
    std::fs::write(&systemctl, "#!/bin/sh\necho policy refused >&2\nexit 7\n").unwrap();
    std::fs::set_permissions(&systemctl, std::fs::Permissions::from_mode(0o700)).unwrap();

    let error = dirs.run_with_path(tools.path(), || {
        install(
            &InstallOptions::builder().apply_schedule(true).build(),
            &bridge(),
        )
        .expect_err("a live scheduler refusal makes the install partial")
    });
    let InstallError::Partial { completed, source } = error else {
        panic!("schedule failure must preserve earlier receipts");
    };
    assert!(matches!(*source, InstallError::ScheduleActivation { .. }));
    assert!(
        completed
            .iter()
            .any(|step| matches!(step, InstallStep::Directory(_)))
    );
    assert!(
        completed
            .iter()
            .any(|step| matches!(step, InstallStep::Sentinel(_)))
    );
    assert!(
        completed
            .iter()
            .any(|step| matches!(step, InstallStep::Policy { .. }))
    );
    assert!(
        !completed
            .iter()
            .any(|step| matches!(step, InstallStep::Schedule { .. }))
    );
    assert!(
        dirs.sentinel().is_file(),
        "completed bootstrap remains durable"
    );

    std::fs::write(&systemctl, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&systemctl, std::fs::Permissions::from_mode(0o700)).unwrap();
    let retried = dirs.run_with_path(tools.path(), || {
        install(
            &InstallOptions::builder().apply_schedule(true).build(),
            &bridge(),
        )
        .expect("retry finishes after scheduler recovery")
    });
    assert!(retried.schedule.is_some());
    assert!(
        dirs.home
            .path()
            .join(".config/systemd/user/systemprompt-bridge-sync.timer")
            .is_file()
    );
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn config_write_failure_reports_durable_bootstrap_and_repair_retry_finishes() {
    use systemprompt_bridge::install::InstallStep;
    use systemprompt_identifiers::ValidatedUrl;

    let dirs = Dirs::new();
    let options = InstallOptions::builder()
        .gateway_url(ValidatedUrl::try_new("https://recovery.example").expect("gateway"))
        .build();

    let (config_file, error) = dirs.run(None, || {
        let config_file = systemprompt_bridge::config::config_path().expect("config path");
        let config_parent = config_file.parent().expect("config parent");
        std::fs::write(config_parent, "operator blocker").expect("block config directory");
        let error =
            install(&options, &bridge()).expect_err("config path failure makes install partial");
        (config_file, error)
    });
    let InstallError::Partial { completed, source } = error else {
        panic!("config write must retain bootstrap receipts");
    };
    assert!(matches!(*source, InstallError::Config(_)));
    assert_eq!(
        completed.len(),
        2,
        "only bootstrap completed: {completed:?}"
    );
    assert!(matches!(completed[0], InstallStep::Directory(_)));
    assert!(matches!(completed[1], InstallStep::Sentinel(_)));
    assert!(dirs.sentinel().is_file(), "version sentinel is durable");
    assert_eq!(
        std::fs::read_to_string(config_file.parent().unwrap()).expect("operator blocker preserved"),
        "operator blocker"
    );

    std::fs::remove_file(config_file.parent().unwrap()).expect("repair config parent");
    let (repaired, persisted_gateway) = dirs.run(None, || {
        let repaired = install(&options, &bridge()).expect("retry succeeds");
        let loaded = systemprompt_bridge::config::load().expect("read persisted config");
        let gateway = systemprompt_bridge::config::gateway_url_or_default(&loaded)
            .as_str()
            .to_owned();
        (repaired, gateway)
    });
    assert!(dirs.org_plugins().is_dir());
    assert!(matches!(
        repaired.mdm,
        systemprompt_bridge::install::MdmDisplay::Snippet { .. }
    ));
    assert_eq!(
        persisted_gateway,
        options.gateway_url.as_ref().unwrap().as_str()
    );
    assert!(config_file.is_file());
}
