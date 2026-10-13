//! Browser-redirect single sign-on provider contract.
//!
//! A provider owns the protocol: [`SsoProvider::start`] builds the redirect to
//! the identity provider and [`SsoProvider::finish`] validates what the
//! identity provider sends back, yielding an [`ExternalIdentity`]. Everything
//! around that — state correlation, replay, the identity policy, provisioning,
//! the session and the audit trail — is the driver's, so every provider gets
//! the same guarantees.
//!
//! `SsoProvider` is held as `Arc<dyn SsoProvider>`, so it uses
//! `#[async_trait]`; native `async fn` in traits is not yet `dyn`-compatible.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
pub mod events;
pub mod policy;

use std::fmt::Debug;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

pub use error::SsoError;
pub use events::{AuthEventSink, ReplayGuard, SsoLoginRefused, SsoLoginSuccess};
pub use policy::{GatedIdentity, SsoIdentityPolicy, SsoRefusal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsoProtocol {
    Saml2,
    Oidc,
}

/// How the identity provider returns the browser to the callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsoReturnBinding {
    Query,
    Form,
}

/// `SameSite` for the state-binder cookie. A form-POST return is a
/// cross-site navigation, so a provider using it needs `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieSameSite {
    Lax,
    None,
}

impl CookieSameSite {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lax => "Lax",
            Self::None => "None",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SsoStart<'a> {
    pub return_to: &'a str,
    pub callback_url: &'a str,
}

/// What the provider needs remembered between `start` and `finish`. `state`
/// is the correlation value the identity provider echoes back (`state` or
/// `RelayState`).
#[derive(Debug, Clone, Default)]
pub struct SsoStateSeed {
    pub state: String,
    pub request_id: Option<String>,
    pub nonce: Option<String>,
    pub pkce_verifier: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SsoRedirect {
    pub url: String,
    pub seed: SsoStateSeed,
}

#[derive(Debug, Clone)]
pub struct SsoReturn {
    pub binding: SsoReturnBinding,
    pub params: Vec<(String, String)>,
}

impl SsoReturn {
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// The remembered half of [`SsoStateSeed`], handed back to `finish`.
#[derive(Debug, Clone, Default)]
pub struct SsoStateContext {
    pub request_id: Option<String>,
    pub nonce: Option<String>,
    pub pkce_verifier: Option<String>,
}

/// A validated identity as the identity provider asserted it.
#[derive(Debug, Clone)]
pub struct ExternalIdentity {
    pub subject: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub display_name: Option<String>,
    pub groups: Vec<String>,
    pub groups_overage: bool,
    pub tenant_id: Option<String>,
    pub assertion_id: Option<String>,
    pub assertion_expires_at: Option<DateTime<Utc>>,
    pub raw_claims: serde_json::Value,
}

/// Paths relative to the API external URL. `callback` is what the identity
/// provider is registered with.
#[derive(Debug, Clone, Copy)]
pub struct SsoPaths {
    pub start: &'static str,
    pub callback: &'static str,
    pub metadata: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct SsoMetadataDoc {
    pub content_type: &'static str,
    pub body: String,
}

#[async_trait]
pub trait SsoProvider: Send + Sync + Debug {
    fn id(&self) -> &str;

    fn issuer(&self) -> &str;

    fn protocol(&self) -> SsoProtocol;

    fn policy(&self) -> &SsoIdentityPolicy;

    fn paths(&self) -> SsoPaths;

    fn login_path(&self) -> &str;

    fn default_return_to(&self) -> &str;

    /// Whether an IdP-initiated return with no matching state may sign in.
    fn allow_unsolicited(&self) -> bool {
        false
    }

    fn return_cookie_same_site(&self) -> CookieSameSite;

    async fn start(&self, input: SsoStart<'_>) -> Result<SsoRedirect, SsoError>;

    async fn finish(
        &self,
        ret: SsoReturn,
        state: Option<SsoStateContext>,
    ) -> Result<ExternalIdentity, SsoError>;

    async fn metadata(&self) -> Option<Result<SsoMetadataDoc, SsoError>> {
        None
    }
}
