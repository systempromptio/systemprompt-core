use std::path::{Path, PathBuf};

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::install::{CredentialsOutcome, uninstall};
use tempfile::TempDir;

struct Sandbox {
    home: TempDir,
    config: TempDir,
    data: TempDir,
    state: TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            home: TempDir::new().expect("home"),
            config: TempDir::new().expect("config"),
            data: TempDir::new().expect("data"),
            state: TempDir::new().expect("state"),
        }
    }

    fn org_plugins(&self) -> PathBuf {
        self.data.path().join("Claude").join("org-plugins")
    }

    fn working_dir(&self) -> PathBuf {
        self.state.path().join("systemprompt-bridge")
    }

    fn metadata(&self) -> PathBuf {
        self.working_dir().join("metadata")
    }

    fn staging(&self) -> PathBuf {
        self.working_dir().join("staging")
    }

    fn pat_file(&self) -> PathBuf {
        self.config
            .path()
            .join("systemprompt")
            .join("systemprompt-bridge.pat")
    }

    fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        let vars: Vec<(&str, Option<String>)> = vec![
            ("HOME", Some(self.home.path().display().to_string())),
            (
                "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
                Some(self.org_plugins().display().to_string()),
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
            ("SUDO_USER", None),
            ("SP_BRIDGE_PAT", None),
            ("SP_BRIDGE_CONFIG", None),
        ];
        temp_env::with_vars(vars, f)
    }
}

fn bridge() -> std::sync::Arc<BridgeContext> {
    BridgeContext::start(ProxyMode::Attach).expect("runtime builds")
}

fn seed_file(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, body).expect("seed");
}

#[test]
fn uninstall_removes_the_metadata_tree_staging_and_installed_plugins() {
    let sandbox = Sandbox::new();
    seed_file(
        &sandbox.metadata().join("version.json"),
        "{\"version\":\"1\"}",
    );
    seed_file(&sandbox.staging().join("half-written.json"), "{}");
    seed_file(
        &sandbox
            .org_plugins()
            .join("acme-plugin")
            .join("plugin.json"),
        "{}",
    );

    let summary = sandbox
        .run(|| uninstall(false, &bridge()))
        .expect("uninstall succeeds in the sandbox");

    assert_eq!(
        summary.metadata_removed.as_deref(),
        Some(sandbox.metadata().as_path()),
        "the metadata dir that existed is reported as removed"
    );
    assert!(summary.metadata_already_clean.is_none());
    assert!(!sandbox.metadata().exists(), "metadata tree is gone");
    assert!(!sandbox.staging().exists(), "staging tree is gone");
    assert!(
        !sandbox.org_plugins().join("acme-plugin").exists(),
        "the installed plugin dir is purged"
    );
    assert!(
        matches!(summary.credentials, CredentialsOutcome::Kept),
        "an uninstall without --purge keeps credentials, got {:?}",
        summary.credentials
    );
}

#[test]
fn uninstall_reports_an_absent_metadata_dir_as_already_clean() {
    let sandbox = Sandbox::new();
    std::fs::create_dir_all(sandbox.org_plugins()).expect("org-plugins root");

    let summary = sandbox
        .run(|| uninstall(false, &bridge()))
        .expect("uninstall succeeds with nothing installed");

    assert!(summary.metadata_removed.is_none());
    assert_eq!(
        summary.metadata_already_clean.as_deref(),
        Some(sandbox.metadata().as_path()),
        "a second uninstall reports the dir it did not need to remove"
    );
}

#[test]
fn uninstall_leaves_dotfiles_and_loose_files_in_the_plugin_root() {
    let sandbox = Sandbox::new();
    seed_file(&sandbox.org_plugins().join(".cache").join("index"), "x");
    seed_file(&sandbox.org_plugins().join("README"), "operator note");
    seed_file(
        &sandbox.org_plugins().join("acme").join("plugin.json"),
        "{}",
    );

    sandbox
        .run(|| uninstall(false, &bridge()))
        .expect("uninstall succeeds");

    assert!(
        sandbox.org_plugins().join(".cache").is_dir(),
        "a dot-prefixed dir is not a plugin and survives"
    );
    assert!(
        sandbox.org_plugins().join("README").is_file(),
        "a loose file is not a plugin dir and survives"
    );
    assert!(!sandbox.org_plugins().join("acme").exists());
}

#[test]
fn purging_removes_the_stored_credential_and_names_the_file() {
    let sandbox = Sandbox::new();
    seed_file(&sandbox.pat_file(), "sp-live-a.b");
    std::fs::create_dir_all(sandbox.org_plugins()).expect("org-plugins root");

    let summary = sandbox
        .run(|| uninstall(true, &bridge()))
        .expect("purging uninstall succeeds");

    let CredentialsOutcome::Purged(pat) = &summary.credentials else {
        panic!(
            "a purge reports the credential file, got {:?}",
            summary.credentials
        );
    };
    assert_eq!(pat.as_path(), sandbox.pat_file().as_path());
    assert!(
        !sandbox.pat_file().exists(),
        "the stored PAT is deleted by a purge"
    );
}
