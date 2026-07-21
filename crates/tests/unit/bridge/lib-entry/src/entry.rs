use systemprompt_bridge::brand::Brand;
use tempfile::TempDir;

#[test]
fn run_with_brand_installs_the_brand_and_purges_legacy_agent_state() {
    let config = TempDir::new().expect("config");
    let state = TempDir::new().expect("state");
    let home = TempDir::new().expect("home");

    let legacy_dir = config.path().join("systemprompt");
    std::fs::create_dir_all(&legacy_dir).expect("legacy config dir");
    let legacy = legacy_dir.join("agents.json");
    std::fs::write(&legacy, "[]").expect("legacy agents state");

    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(home.path().display().to_string())),
        ("XDG_CONFIG_HOME", Some(config.path().display().to_string())),
        ("XDG_STATE_HOME", Some(state.path().display().to_string())),
        ("XDG_CACHE_HOME", Some(home.path().display().to_string())),
        ("XDG_DATA_HOME", Some(home.path().display().to_string())),
        ("SP_BRIDGE_CONFIG", None),
        ("SP_BRIDGE_PAT", None),
    ];
    temp_env::with_vars(vars, || {
        let _ = systemprompt_bridge::run_with_brand(&Brand::SYSTEMPROMPT);
    });

    assert!(
        !legacy.exists(),
        "process start purges the legacy agents.json"
    );
    assert_eq!(
        systemprompt_bridge::brand::brand().binary_name,
        Brand::SYSTEMPROMPT.binary_name,
        "the brand is installed process-wide before anything reads it"
    );
}
