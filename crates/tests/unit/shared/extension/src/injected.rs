use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_extension::runtime_config::{InjectedExtensions, WebAssetsStrategy};
use systemprompt_extension::{Extension, ExtensionMetadata, ExtensionRouterConfig};

struct StubExt;

impl Extension for StubExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "stub",
            name: "Stub",
            version: "0.0.1",
        }
    }
}

#[test]
fn injected_extensions_default_is_empty() {
    let inj = InjectedExtensions::default();
    assert!(inj.extensions.is_empty());
    assert!(matches!(inj.web_assets, WebAssetsStrategy::Disabled));
}

#[test]
fn injected_extensions_debug_shows_count() {
    let inj = InjectedExtensions {
        extensions: vec![Arc::new(StubExt)],
        web_assets: WebAssetsStrategy::Disabled,
    };
    let debug = format!("{inj:?}");
    assert!(debug.contains("InjectedExtensions"));
    assert!(debug.contains("1"));
}

#[test]
fn injected_extensions_debug_file_path_variant() {
    let inj = InjectedExtensions {
        extensions: vec![],
        web_assets: WebAssetsStrategy::FilePath(PathBuf::from("/assets")),
    };
    let debug = format!("{inj:?}");
    assert!(debug.contains("FilePath"));
}

#[test]
fn injected_extensions_debug_remote_variant() {
    let inj = InjectedExtensions {
        extensions: vec![],
        web_assets: WebAssetsStrategy::Remote {
            url: "https://cdn.example.com".to_string(),
            cache_dir: PathBuf::from("/tmp/cache"),
        },
    };
    let debug = format!("{inj:?}");
    assert!(debug.contains("Remote"));
}

#[test]
fn web_assets_strategy_file_path_stores_path() {
    let path = PathBuf::from("/var/www/assets");
    let strategy = WebAssetsStrategy::FilePath(path.clone());
    match strategy {
        WebAssetsStrategy::FilePath(p) => assert_eq!(p, path),
        _ => panic!("unexpected variant"),
    }
}

#[test]
fn web_assets_strategy_remote_stores_fields() {
    let strategy = WebAssetsStrategy::Remote {
        url: "https://cdn.test.com".to_string(),
        cache_dir: PathBuf::from("/cache"),
    };
    match &strategy {
        WebAssetsStrategy::Remote { url, cache_dir } => {
            assert_eq!(url, "https://cdn.test.com");
            assert_eq!(cache_dir, &PathBuf::from("/cache"));
        },
        _ => panic!("unexpected variant"),
    }
}

#[test]
fn web_assets_strategy_clone_file_path() {
    let strategy = WebAssetsStrategy::FilePath(PathBuf::from("/x"));
    let cloned = strategy.clone();
    assert!(matches!(cloned, WebAssetsStrategy::FilePath(_)));
}

#[test]
fn web_assets_strategy_clone_remote() {
    let strategy = WebAssetsStrategy::Remote {
        url: "https://a.b".to_string(),
        cache_dir: PathBuf::from("/y"),
    };
    let cloned = strategy.clone();
    assert!(matches!(cloned, WebAssetsStrategy::Remote { .. }));
}

#[test]
fn router_config_clone() {
    let config = ExtensionRouterConfig::new("/api/v2/cloned");
    let cloned = config;
    assert_eq!(cloned.base_path, "/api/v2/cloned");
    assert!(cloned.requires_auth);
}

#[test]
fn router_config_public_clone() {
    let config = ExtensionRouterConfig::public("/api/v2/pub");
    let cloned = config;
    assert_eq!(cloned.base_path, "/api/v2/pub");
    assert!(!cloned.requires_auth);
}
