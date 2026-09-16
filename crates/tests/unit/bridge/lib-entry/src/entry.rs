use systemprompt_bridge::brand::Brand;
use tempfile::TempDir;

#[test]
fn run_with_brand_installs_the_brand_and_leaves_foreign_state_alone() {
    let config = TempDir::new().expect("config");
    let state = TempDir::new().expect("state");
    let home = TempDir::new().expect("home");

    let config_dir = config.path().join("systemprompt");
    std::fs::create_dir_all(&config_dir).expect("config dir");
    let foreign = config_dir.join("agents.json");
    std::fs::write(&foreign, "[]").expect("a file the bridge did not write");

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
        foreign.exists(),
        "process start removes nothing it has no record of writing"
    );
    assert_eq!(
        systemprompt_bridge::brand::brand().binary_name,
        Brand::SYSTEMPROMPT.binary_name,
        "the brand is installed process-wide before anything reads it"
    );
}
