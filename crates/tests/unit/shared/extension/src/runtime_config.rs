use std::path::PathBuf;

use systemprompt_extension::runtime_config::{
    InjectedExtensions, WebAssetsStrategy, get_injected_extensions, get_web_assets_strategy,
    set_injected_extensions,
};

use crate::injected_lock::{self, ASSETS_PATH, PRIMARY_ID, SECONDARY_ID};

// Why: asserting the pre-set state is not possible here — the lock is global to
// the test binary and another module may legitimately have set it first. What
// survives the race is the invariant that matters: after a set, the getters
// report that payload and every later set is refused.
#[test]
fn injected_extensions_lock_sets_once_and_refuses_the_rest() {
    injected_lock::ensure_set();

    let ids: Vec<_> = get_injected_extensions()
        .iter()
        .map(|e| e.id().to_owned())
        .collect();
    assert!(ids.contains(&PRIMARY_ID.to_owned()));
    assert!(ids.contains(&SECONDARY_ID.to_owned()));

    match get_web_assets_strategy() {
        WebAssetsStrategy::FilePath(p) => assert_eq!(p, PathBuf::from(ASSETS_PATH)),
        other => panic!("expected FilePath strategy, got {other:?}"),
    }

    assert!(
        set_injected_extensions(InjectedExtensions::default()).is_err(),
        "the injected-extensions OnceLock must reject a second set"
    );
}

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
