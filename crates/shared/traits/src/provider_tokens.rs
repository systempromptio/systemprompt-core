//! [`ProviderTokenCache`]: the handle an extension receives for the minted
//! upstream-token cache the platform owns.
//!
//! The cache itself lives in the security crate; this trait lets the
//! extension context hand it out without the shared layer naming that type.
//! The crate that owns the concrete cache recovers it with
//! [`ProviderTokenCache::into_any`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

pub trait ProviderTokenCache: Any + Send + Sync + Debug {
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
}
