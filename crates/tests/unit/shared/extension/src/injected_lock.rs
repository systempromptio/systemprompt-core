//! The injected-extensions payload shared by the discovery tests.
//!
//! The payload is a plain value handed to `ExtensionRegistry::discover_with`;
//! every test builds its own, so nothing is shared across the binary.

use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_extension::runtime_config::{InjectedExtensions, WebAssetsStrategy};
use systemprompt_extension::{Extension, ExtensionMetadata};

pub(crate) const PRIMARY_ID: &str = "inj-primary";
pub(crate) const SECONDARY_ID: &str = "inj-secondary";
pub(crate) const ASSETS_PATH: &str = "/srv/assets";

pub(crate) struct NamedExt {
    pub(crate) id: &'static str,
}

impl Extension for NamedExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: self.id,
            name: "Injected",
            version: "1.0.0",
        }
    }
}

// Why: the duplicate `PRIMARY_ID` is load-bearing — discovery asserts that a
// repeated injected id is skipped rather than double-counted.
pub(crate) fn injected() -> InjectedExtensions {
    InjectedExtensions {
        extensions: vec![
            Arc::new(NamedExt { id: PRIMARY_ID }),
            Arc::new(NamedExt { id: PRIMARY_ID }),
            Arc::new(NamedExt { id: SECONDARY_ID }),
        ],
        web_assets: WebAssetsStrategy::FilePath(PathBuf::from(ASSETS_PATH)),
    }
}
