use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_extension::runtime_config::{
    InjectedExtensions, WebAssetsStrategy, get_injected_extensions, get_web_assets_strategy,
    set_injected_extensions,
};
use systemprompt_extension::{Extension, ExtensionMetadata};

struct RcExt;

impl Extension for RcExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "rc-inj",
            name: "Runtime Config Injected",
            version: "1.0.0",
        }
    }
}

#[test]
fn get_injected_extensions_is_empty_before_any_set() {
    // Fresh process under cargo-nextest: the OnceLock is unset, so the getter
    // falls back to an empty list rather than panicking.
    assert!(get_injected_extensions().is_empty());
}

#[test]
fn get_web_assets_strategy_is_disabled_before_any_set() {
    assert!(matches!(
        get_web_assets_strategy(),
        WebAssetsStrategy::Disabled
    ));
}

#[test]
fn set_injected_extensions_populates_both_getters() {
    set_injected_extensions(InjectedExtensions {
        extensions: vec![Arc::new(RcExt)],
        web_assets: WebAssetsStrategy::FilePath(PathBuf::from("/srv/assets")),
    })
    .expect("first set succeeds");

    let exts = get_injected_extensions();
    assert_eq!(exts.len(), 1);
    assert_eq!(exts[0].id(), "rc-inj");

    match get_web_assets_strategy() {
        WebAssetsStrategy::FilePath(p) => assert_eq!(p, PathBuf::from("/srv/assets")),
        other => panic!("expected FilePath strategy, got {other:?}"),
    }
}

#[test]
fn set_injected_extensions_rejects_a_second_set() {
    set_injected_extensions(InjectedExtensions::default()).expect("first set succeeds");
    let second = set_injected_extensions(InjectedExtensions::default());
    assert!(
        second.is_err(),
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
