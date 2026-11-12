//! Bearer tokens for Cloud Storage.
//!
//! The storage crate never mints a credential itself: a [`GcsTokenSource`] is
//! injected by the composition root, which links the security crate and
//! wraps its one Google token implementation — a service-account key or the
//! runtime's workload identity — behind this trait.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;

use super::GcsError;

/// A source of OAuth bearer tokens for the Cloud Storage JSON API.
///
/// Held as `Arc<dyn GcsTokenSource>` by the backend, so it uses
/// `#[async_trait]` for `dyn`-compatibility.
#[async_trait]
pub trait GcsTokenSource: Send + Sync {
    async fn bearer(&self) -> Result<String, GcsError>;
}
