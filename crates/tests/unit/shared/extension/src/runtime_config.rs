use systemprompt_extension::runtime_config::WebAssetsStrategy;

#[test]
fn web_assets_strategy_disabled_is_default() {
    let strategy = WebAssetsStrategy::default();
    assert!(matches!(strategy, WebAssetsStrategy::Disabled));
}

#[test]
fn web_assets_strategy_file_path_variant() {
    let strategy = WebAssetsStrategy::FilePath(std::path::PathBuf::from("/var/www/assets"));
    assert!(matches!(strategy, WebAssetsStrategy::FilePath(_)));
}

#[test]
fn web_assets_strategy_remote_variant() {
    let strategy = WebAssetsStrategy::Remote {
        url: "https://cdn.example.com".to_string(),
        cache_dir: std::path::PathBuf::from("/tmp/cache"),
    };
    assert!(matches!(strategy, WebAssetsStrategy::Remote { .. }));
}

#[test]
fn web_assets_strategy_debug_format() {
    let strategy = WebAssetsStrategy::Disabled;
    let debug = format!("{strategy:?}");
    assert!(debug.contains("Disabled"));
}

#[test]
fn web_assets_strategy_clone() {
    let strategy = WebAssetsStrategy::FilePath(std::path::PathBuf::from("/assets"));
    let cloned = strategy.clone();
    assert!(
        matches!(cloned, WebAssetsStrategy::FilePath(ref p) if p.to_str().unwrap() == "/assets")
    );
}
