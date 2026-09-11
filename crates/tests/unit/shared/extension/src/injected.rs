use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_extension::runtime_config::{InjectedExtensions, WebAssetsStrategy};
use systemprompt_extension::{Extension, ExtensionMetadata};

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
