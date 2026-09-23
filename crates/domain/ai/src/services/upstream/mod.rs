//! The one way this crate and the gateway address an upstream provider.
//!
//! A catalog [`ProviderEntry`](systemprompt_models::services::providers::ProviderEntry) names an endpoint template and the secret that
//! authenticates it. [`UpstreamTarget::resolve`] turns the pair into something
//! a request can be sent to: the secret parsed into a
//! [`ProviderCredential`](systemprompt_security::credential::ProviderCredential),
//! the endpoint filled from the credential's scope (`{project}` from a Google
//! service account), and the hosting read off the endpoint host. Per request,
//! [`UpstreamTarget::call`] mints the auth header — a verbatim API key, or a
//! Google OAuth token refreshed from the shared cache — and yields an
//! [`UpstreamCall`], which renders the URL, headers and body envelope for a
//! given wire through [`UpstreamDialect`].
//!
//! Both the gateway's outbound adapters and the in-process provider clients go
//! through here, so a platform either stack can reach, the other can too.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod call;
mod error;
mod target;

pub use call::UpstreamCall;
pub use error::UpstreamTargetError;
pub use systemprompt_models::wire::upstream::UpstreamDialect;
pub use target::UpstreamTarget;
