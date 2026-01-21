#[cfg(feature = "web")]
use axum::Router;

use crate::types::ExtensionMeta;

pub trait ApiExtensionTyped: ExtensionMeta {
    fn base_path(&self) -> &'static str;

    fn requires_auth(&self) -> bool {
        true
    }
}

#[cfg(feature = "web")]
pub trait ApiExtensionTypedDyn: ApiExtensionTyped {
    fn build_router(&self) -> Router;
}
