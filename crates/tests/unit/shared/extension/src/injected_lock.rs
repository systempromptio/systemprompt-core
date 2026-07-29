//! Shared ownership of the process-global injected-extensions `OnceLock`.
//!
//! The lock is settable once per process, so every test in this binary that
//! depends on it has to agree on a single payload and a single set. Without
//! that agreement the tests race, and whichever runs first decides which of the
//! others fail.

use std::path::PathBuf;
use std::sync::{Arc, Once};

use systemprompt_extension::runtime_config::{
    InjectedExtensions, WebAssetsStrategy, set_injected_extensions,
};
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

static INIT: Once = Once::new();

// Why: the duplicate `PRIMARY_ID` is load-bearing — discovery asserts that a
// repeated injected id is skipped rather than double-counted.
pub(crate) fn ensure_set() {
    INIT.call_once(|| {
        set_injected_extensions(InjectedExtensions {
            extensions: vec![
                Arc::new(NamedExt { id: PRIMARY_ID }),
                Arc::new(NamedExt { id: PRIMARY_ID }),
                Arc::new(NamedExt { id: SECONDARY_ID }),
            ],
            web_assets: WebAssetsStrategy::FilePath(PathBuf::from(ASSETS_PATH)),
        })
        .expect("injected extensions may be set exactly once per process");
    });
}
