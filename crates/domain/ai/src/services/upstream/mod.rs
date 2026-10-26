//! The one way this crate and the gateway address an upstream endpoint.
//!
//! A [`ResolvedHop`](systemprompt_manifest::services::catalog::ResolvedHop)
//! names an endpoint (wire, URL, betas, hosting), its provider (headers, user
//! agent) and the provider's credential. [`UpstreamTarget::resolve`] turns
//! the hop plus the credential's secret into something a request can be sent
//! to: the secret parsed into a
//! [`ProviderCredential`](systemprompt_security::credential::ProviderCredential),
//! the URL filled from the credential's scope (`{project}` from a Google
//! service account), and the token cache key taken from the credential id.
//! Per request, [`UpstreamTarget::call`] mints the auth header — a verbatim
//! API key, or a Google OAuth token from the credential-keyed cache — and
//! yields an [`UpstreamCall`], which renders the URL, headers and body
//! envelope for a given wire through [`UpstreamDialect`].
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
pub use systemprompt_wire::upstream::UpstreamDialect;
pub use target::{EndpointBinding, UpstreamTarget};
