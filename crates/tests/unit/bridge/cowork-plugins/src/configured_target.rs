use std::path::{Path, PathBuf};
use systemprompt_bridge::integration::cowork_plugins::{
    CoworkTarget, apply_enable, clear_all, resolve_target,
};
use tempfile::TempDir;

struct Sandbox {
    dir: TempDir,
}

impl Sandbox {
    fn new() -> Self {
        Self {
            dir: TempDir::new().expect("tempdir"),
        }
    }

    fn org_dir(&self, name: &str, with_plugins_subdir: bool) -> PathBuf {
        let dir = self.dir.path().join(name);
        if with_plugins_subdir {
            std::fs::create_dir_all(dir.join("cowork_plugins")).expect("plugins subdir");
        } else {
            std::fs::create_dir_all(&dir).expect("org dir");
        }
        dir
    }

    fn with_config<R>(&self, session_org_dir: &Path, f: impl FnOnce() -> R) -> R {
        let config_home = self.dir.path().join("config");
        std::fs::create_dir_all(config_home.join("systemprompt")).expect("config dir");
        std::fs::write(
            config_home
                .join("systemprompt")
                .join("systemprompt-bridge.toml"),
            format!(
                "gateway_url = \"http://gw.invalid:7000\"\n\n[cowork]\nsession_org_dir = \"{}\"\n",
                session_org_dir.display()
            ),
        )
        .expect("config file");
        let vars: Vec<(&str, Option<String>)> = vec![
            ("XDG_CONFIG_HOME", Some(config_home.display().to_string())),
            ("HOME", Some(self.dir.path().display().to_string())),
            ("SP_BRIDGE_CONFIG", None),
        ];
        temp_env::with_vars(vars, f)
    }
}

#[test]
fn a_configured_session_dir_overrides_the_filesystem_scan() {
    let sb = Sandbox::new();
    let chosen = sb.org_dir("chosen-org", true);
    let target = sb
        .with_config(&chosen, resolve_target)
        .expect("the configured dir is used");
    assert_eq!(target.session_org_dir, chosen);
    assert_eq!(target.cowork_plugins_dir, chosen.join("cowork_plugins"));
}

#[test]
fn a_configured_session_dir_without_a_plugins_subdir_is_refused() {
    let sb = Sandbox::new();
    let half = sb.org_dir("half-initialised", false);
    assert!(
        sb.with_config(&half, resolve_target).is_none(),
        "a configured dir with no cowork_plugins subdir must not fall back to guessing"
    );
}

fn target_at(dir: &Path) -> CoworkTarget {
    CoworkTarget {
        session_org_dir: dir.to_path_buf(),
        cowork_plugins_dir: dir.join("cowork_plugins"),
    }
}

#[test]
fn legacy_state_files_of_the_wrong_shape_are_left_untouched() {
    let sb = Sandbox::new();
    let dir = sb.org_dir("org", true);
    let plugins = dir.join("cowork_plugins");
    let installed = plugins.join("installed_plugins.json");
    let known = plugins.join("known_marketplaces.json");
    std::fs::write(&installed, r#"["not","an","object"]"#).expect("installed");
    std::fs::write(&known, r#"["also","not","an","object"]"#).expect("known");

    apply_enable(&target_at(&dir), &["acme-plugin"]).expect("apply tolerates foreign shapes");

    assert_eq!(
        std::fs::read_to_string(&installed).expect("installed"),
        r#"["not","an","object"]"#,
        "a non-object installed_plugins.json is preserved verbatim"
    );
    assert_eq!(
        std::fs::read_to_string(&known).expect("known"),
        r#"["also","not","an","object"]"#,
        "a non-object known_marketplaces.json is preserved verbatim"
    );
}

#[test]
fn legacy_state_without_the_purged_keys_is_left_unchanged() {
    let sb = Sandbox::new();
    let dir = sb.org_dir("org", true);
    let plugins = dir.join("cowork_plugins");
    let installed = plugins.join("installed_plugins.json");
    let known = plugins.join("known_marketplaces.json");
    std::fs::write(&installed, r#"{"plugins":{"other@elsewhere":{}}}"#).expect("installed");
    std::fs::write(&known, r#"{"other-marketplace":{}}"#).expect("known");

    clear_all(&target_at(&dir)).expect("clear succeeds");

    let installed_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&installed).expect("installed"))
            .expect("json");
    assert!(
        installed_json["plugins"]["other@elsewhere"].is_object(),
        "an unrelated installed plugin survives the legacy purge: {installed_json}"
    );
    let known_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&known).expect("known")).expect("json");
    assert!(
        known_json["other-marketplace"].is_object(),
        "an unrelated marketplace survives the legacy purge: {known_json}"
    );
}

#[test]
fn applying_no_plugins_reports_nothing_enabled() {
    let sb = Sandbox::new();
    let dir = sb.org_dir("org", true);
    let report = apply_enable(&target_at(&dir), &[]).expect("apply succeeds");
    assert_eq!(report.target.as_deref(), Some(dir.as_path()));
    assert!(
        !report.enabled,
        "an empty plugin set enables nothing, but still reports its target"
    );
}
