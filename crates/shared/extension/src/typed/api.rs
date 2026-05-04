//! [`ApiExtensionTyped`] — typed contract for extensions that mount an
//! axum router.

use axum::Router;

use crate::types::ExtensionMeta;

/// Typed contract for an extension that mounts an axum router.
pub trait ApiExtensionTyped: ExtensionMeta {
    /// Returns the static base path the router mounts at (must start with
    /// `/api/`).
    fn base_path(&self) -> &'static str;

    /// Returns true if the router requires an authenticated request.
    fn requires_auth(&self) -> bool {
        true
    }
}

/// Object-safe variant of [`ApiExtensionTyped`] that exposes a
/// `build_router()` method. Implement this when the host needs to store
/// API extensions behind a `dyn` reference.
pub trait ApiExtensionTypedDyn: ApiExtensionTyped {
    /// Builds the axum router for this extension.
    fn build_router(&self) -> Router;
}
