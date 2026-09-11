use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use systemprompt_bridge::cli::doctor::Status;
use systemprompt_bridge::cli::doctor::filesystem::{
    check_bridge_working_dir, check_org_plugins_writable, check_private_files,
};
use tempfile::TempDir;

struct Sandbox {
    home: TempDir,
    config: TempDir,
    data: TempDir,
    state: TempDir,
    system_org_plugins: PathBuf,
}

impl Sandbox {
    fn new(system_org_plugins: PathBuf) -> Self {
        Self {
            home: TempDir::new().expect("home"),
            config: TempDir::new().expect("config"),
            data: TempDir::new().expect("data"),
            state: TempDir::new().expect("state"),
            system_org_plugins,
        }
    }

    fn config_file(&self) -> PathBuf {
        self.config
            .path()
            .join("systemprompt")
            .join("systemprompt-bridge.toml")
    }

    fn user_org_plugins(&self) -> PathBuf {
        self.data.path().join("Claude").join("org-plugins")
    }

    fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        let vars: Vec<(&str, Option<String>)> = vec![
            ("HOME", Some(self.home.path().display().to_string())),
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
                "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
                Some(self.system_org_plugins.display().to_string()),
            ),
            ("SP_BRIDGE_CONFIG", None),
        ];
        temp_env::with_vars(vars, f)
    }
}

fn chmod(path: &std::path::Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
}

#[test]
fn the_private_files_check_passes_when_every_seeded_file_opens() {
    let unwritable = TempDir::new().expect("system decoy");
    let sandbox = Sandbox::new(unwritable.path().join("Claude").join("org-plugins"));
    std::fs::create_dir_all(sandbox.config_file().parent().expect("parent")).expect("mkdir");
    std::fs::write(
        sandbox.config_file(),
        "gateway_url = \"https://g.example\"\n",
    )
    .expect("seed config");

    let check = sandbox.run(check_private_files);

    assert_eq!(check.status, Status::Ok, "{}", check.detail);
    assert!(
        check.detail.contains("opens for this user"),
        "{}",
        check.detail
    );
}

#[test]
fn the_private_files_check_names_a_file_this_user_cannot_open() {
    let unwritable = TempDir::new().expect("system decoy");
    let sandbox = Sandbox::new(unwritable.path().join("Claude").join("org-plugins"));
    std::fs::create_dir_all(sandbox.config_file().parent().expect("parent")).expect("mkdir");
    std::fs::write(
        sandbox.config_file(),
        "gateway_url = \"https://g.example\"\n",
    )
    .expect("seed config");
    chmod(&sandbox.config_file(), 0o000);

    let check = sandbox.run(check_private_files);

    chmod(&sandbox.config_file(), 0o600);
    assert_eq!(check.status, Status::Fail, "{}", check.detail);
    assert!(
        check.detail.contains("systemprompt-bridge.toml"),
        "the unreadable file is named: {}",
        check.detail
    );
    assert!(
        check.detail.contains("start the bridge"),
        "the operator is told how to repair it: {}",
        check.detail
    );
}

#[test]
fn the_working_dir_check_fails_when_the_state_root_cannot_be_created_in() {
    let unwritable = TempDir::new().expect("system decoy");
    let sandbox = Sandbox::new(unwritable.path().join("Claude").join("org-plugins"));
    chmod(sandbox.state.path(), 0o500);

    let check = sandbox.run(check_bridge_working_dir);

    chmod(sandbox.state.path(), 0o700);
    assert_eq!(check.status, Status::Fail, "{}", check.detail);
    assert!(
        check.detail.contains("cannot create staging"),
        "the failing step and path are named: {}",
        check.detail
    );
}

#[test]
fn the_org_plugins_check_fails_when_the_effective_root_is_not_writable() {
    let system = TempDir::new().expect("system root");
    chmod(system.path(), 0o500);
    let sandbox = Sandbox::new(system.path().join("Claude").join("org-plugins"));
    std::fs::create_dir_all(sandbox.user_org_plugins()).expect("user org-plugins");
    chmod(&sandbox.user_org_plugins(), 0o500);

    let check = sandbox.run(check_org_plugins_writable);

    chmod(&sandbox.user_org_plugins(), 0o700);
    chmod(system.path(), 0o700);
    assert_eq!(check.status, Status::Fail, "{}", check.detail);
    assert!(check.detail.contains("is NOT writable"), "{}", check.detail);
    assert!(
        check.detail.contains("install --apply"),
        "the repair command is offered: {}",
        check.detail
    );
}
