//! [`ApiExtensionTyped`] — typed contract for extensions that mount an
//! axum router.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;

use crate::types::ExtensionMeta;

pub trait ApiExtensionTyped: ExtensionMeta {
    fn base_path(&self) -> &'static str;

    fn requires_auth(&self) -> bool {
        true
    }
}

pub trait ApiExtensionTypedDyn: ApiExtensionTyped {
    fn build_router(&self) -> Router;
}
