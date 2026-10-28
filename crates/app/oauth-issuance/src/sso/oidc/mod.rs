//! A generic `OpenID Connect` relying party: the authorization-code flow with
//! PKCE (S256), `state` and `nonce`, validated against the issuer's discovery
//! document and JWKS.
//!
//! It is configured by `services/web/config/oidc.yaml` and registered with
//! the SSO driver like any other provider, so an install without that file,
//! or with `enabled: false`, offers no new sign-in. One OIDC client at one
//! issuer per install: a central Keycloak serves many installs by giving each
//! its own client in a shared realm. Correlation, replay, the identity
//! policy, provisioning and the session are the driver's; this module builds
//! the redirect and turns the callback into an [`ExternalIdentity`].
//!
//! The client secret is read from the secrets store by key at every code
//! redemption, so a rotated secret takes effect on the next sign-in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod discovery;
mod error;
mod token;
mod verify;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use systemprompt_client::{GuardedClientConfig, guarded_client};
use systemprompt_security::keys::JwksClient;
use systemprompt_traits::SecretStore;
use systemprompt_traits::sso::{
    CookieSameSite, ExternalIdentity, SsoError, SsoIdentityPolicy, SsoPaths, SsoProtocol,
    SsoProvider, SsoRedirect, SsoReturn, SsoStart, SsoStateContext, SsoStateSeed,
};
use uuid::Uuid;

pub use config::{CONFIG_FILE, OidcClaimMapping, OidcConfig, OidcConfigError};
pub use discovery::OidcDiscovery;
pub use error::OidcProviderError;
pub use verify::{identity_from_claims, validate_id_token};

use super::registry::{SsoHookContext, SsoProviderError};
use discovery::DiscoveryCache;
use token::{CodeRedemption, redeem_code};

pub const ID: &str = "oidc";

const PATHS: SsoPaths = SsoPaths {
    start: "/api/v1/core/sso/oidc/start",
    callback: "/api/v1/core/sso/oidc/callback",
    metadata: None,
};

const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

/// The generic OIDC provider built from an enabled, validated [`OidcConfig`].
pub struct OidcProvider {
    cfg: OidcConfig,
    secrets: Arc<dyn SecretStore>,
    http: reqwest::Client,
    discovery: DiscoveryCache,
    jwks: JwksClient,
    redirect_uri: String,
}

impl std::fmt::Debug for OidcProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcProvider")
            .field("issuer", &self.cfg.issuer())
            .field("client_id", &self.cfg.client_id)
            .finish_non_exhaustive()
    }
}

impl OidcProvider {
    pub fn new(
        cfg: OidcConfig,
        secrets: Arc<dyn SecretStore>,
        api_external_url: &str,
    ) -> Result<Self, OidcConfigError> {
        cfg.validate(secrets.as_ref())?;
        let http = guarded_client(
            &GuardedClientConfig::default()
                .with_timeout(HTTP_TIMEOUT)
                .with_max_redirects(0),
        )
        .map_err(OidcConfigError::HttpClient)?;
        let discovery = DiscoveryCache::new(cfg.discovery_url(), cfg.issuer().to_owned());
        let jwks = JwksClient::new(cfg.jwks_hosts());
        let redirect_uri = format!(
            "{}{}",
            api_external_url.trim_end_matches('/'),
            PATHS.callback
        );
        Ok(Self {
            cfg,
            secrets,
            http,
            discovery,
            jwks,
            redirect_uri,
        })
    }

    fn client_secret(&self) -> Result<String, SsoError> {
        self.secrets
            .secret(&self.cfg.client_secret_key)
            .map(|secret| secret.expose().to_owned())
            .filter(|secret| !secret.is_empty())
            .ok_or_else(|| {
                SsoError::internal(OidcConfigError::MissingSecret {
                    key: self.cfg.client_secret_key.clone(),
                })
            })
    }
}

pub fn build(ctx: &SsoHookContext) -> Result<Option<OidcProvider>, OidcConfigError> {
    let Some(cfg) = OidcConfig::load(&ctx.services_root.path)? else {
        return Ok(None);
    };
    if !cfg.enabled {
        return Ok(None);
    }
    OidcProvider::new(cfg, Arc::clone(&ctx.secrets), &ctx.config.api_external_url).map(Some)
}

fn factory(ctx: &SsoHookContext) -> Result<Option<Arc<dyn SsoProvider>>, SsoProviderError> {
    let Some(provider) = build(ctx).map_err(|source| SsoProviderError::config(ID, source))? else {
        return Ok(None);
    };
    tracing::info!(
        issuer = provider.cfg.issuer(),
        "Generic OIDC SSO provider enabled"
    );
    Ok(Some(Arc::new(provider)))
}

crate::register_sso_provider!(factory);

fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

#[must_use]
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[derive(Debug, Clone, Copy)]
pub struct AuthorizeParams<'a> {
    pub redirect_uri: &'a str,
    pub state: &'a str,
    pub nonce: &'a str,
    pub code_challenge: &'a str,
}

#[must_use]
pub fn build_authorize_url(
    authorization_endpoint: &str,
    cfg: &OidcConfig,
    params: AuthorizeParams<'_>,
) -> String {
    let AuthorizeParams {
        redirect_uri,
        state,
        nonce,
        code_challenge,
    } = params;
    let scope = cfg.scopes.join(" ");
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", &cfg.client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", &scope)
        .append_pair("state", state)
        .append_pair("nonce", nonce)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256")
        .finish();
    let separator = if authorization_endpoint.contains('?') {
        '&'
    } else {
        '?'
    };
    format!("{authorization_endpoint}{separator}{query}")
}

#[async_trait]
impl SsoProvider for OidcProvider {
    fn id(&self) -> &str {
        ID
    }

    fn issuer(&self) -> &str {
        self.cfg.issuer()
    }

    fn protocol(&self) -> SsoProtocol {
        SsoProtocol::Oidc
    }

    fn policy(&self) -> &SsoIdentityPolicy {
        &self.cfg.policy
    }

    fn paths(&self) -> SsoPaths {
        PATHS
    }

    fn login_path(&self) -> &str {
        &self.cfg.login_path
    }

    fn default_return_to(&self) -> &str {
        &self.cfg.default_return_to
    }

    fn return_cookie_same_site(&self) -> CookieSameSite {
        CookieSameSite::Lax
    }

    async fn start(&self, input: SsoStart<'_>) -> Result<SsoRedirect, SsoError> {
        let discovery = self.discovery.current(&self.http).await?;
        let state = random_token();
        let nonce = random_token();
        let verifier = random_token();
        let url = build_authorize_url(
            &discovery.authorization_endpoint,
            &self.cfg,
            AuthorizeParams {
                redirect_uri: input.callback_url,
                state: &state,
                nonce: &nonce,
                code_challenge: &pkce_challenge(&verifier),
            },
        );
        Ok(SsoRedirect {
            url,
            seed: SsoStateSeed {
                state,
                request_id: None,
                nonce: Some(nonce),
                pkce_verifier: Some(verifier),
            },
        })
    }

    async fn finish(
        &self,
        ret: SsoReturn,
        state: Option<SsoStateContext>,
    ) -> Result<ExternalIdentity, SsoError> {
        let state = state.ok_or(SsoError::StateLost)?;
        if let Some(code) = ret.param("error") {
            return Err(OidcProviderError::IdpError(code.to_owned()).into());
        }
        let code = ret.param("code").ok_or(OidcProviderError::NoCode)?;
        let (Some(nonce), Some(verifier)) = (state.nonce, state.pkce_verifier) else {
            return Err(SsoError::StateLost);
        };
        let discovery = self.discovery.current(&self.http).await?;
        // Why: RFC 9207 §2.4 — an `iss` on the authorization response must
        // name the issuer the request went to.
        if let Some(iss) = ret.param("iss")
            && iss.trim_end_matches('/') != self.cfg.issuer()
        {
            return Err(OidcProviderError::ResponseIssuer.into());
        }
        let client_secret = self.client_secret()?;
        let id_token = redeem_code(
            &self.http,
            &CodeRedemption {
                token_endpoint: &discovery.token_endpoint,
                code,
                redirect_uri: &self.redirect_uri,
                client_id: &self.cfg.client_id,
                client_secret: &client_secret,
                code_verifier: &verifier,
            },
        )
        .await?;
        Ok(validate_id_token(&id_token, &self.cfg, &discovery, &self.jwks, &nonce).await?)
    }
}
