use std::path::PathBuf;

use systemprompt_extension::ExtensionRegistry;
use systemprompt_extension::runtime_config::{InjectedExtensions, WebAssetsStrategy};

use crate::injected_lock::{self, ASSETS_PATH, PRIMARY_ID, SECONDARY_ID};

#[test]
fn injected_extensions_are_a_value_each_discovery_takes_as_an_argument() {
    let injected = injected_lock::injected();

    let ids: Vec<_> = injected
        .extensions
        .iter()
        .map(|e| e.id().to_owned())
        .collect();
    assert!(ids.contains(&PRIMARY_ID.to_owned()));
    assert!(ids.contains(&SECONDARY_ID.to_owned()));

    match &injected.web_assets {
        WebAssetsStrategy::FilePath(p) => assert_eq!(p, &PathBuf::from(ASSETS_PATH)),
        other => panic!("expected FilePath strategy, got {other:?}"),
    }

    let with = ExtensionRegistry::discover_with(&injected).expect("discover with injected");
    let without =
        ExtensionRegistry::discover_with(&InjectedExtensions::default()).expect("discover");
    assert!(with.has(PRIMARY_ID));
    assert!(
        !without.has(PRIMARY_ID),
        "an injected list is scoped to the call that passes it, not the process"
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
fn web_assets_strategy_clone() {
    let strategy = WebAssetsStrategy::FilePath(std::path::PathBuf::from("/assets"));
    let cloned = strategy.clone();
    assert!(
        matches!(cloned, WebAssetsStrategy::FilePath(ref p) if p.to_str().unwrap() == "/assets")
    );
}
