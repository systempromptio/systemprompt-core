//! Machine grants for an MCP connector: a token for a deployment's own
//! identity, minted without any user's consent.
//!
//! A connector's `grant` decides how its token is obtained. The consent flow
//! (`authorization_code`) belongs to the extension that banks per-user
//! tokens; the two machine grants live here because they need nothing but
//! configuration and secrets. `client_credentials` posts the client id and
//! secret to `token_url`. `app_installation` signs an RS256 app JWT and
//! exchanges it for an installation token (GitHub Apps). Either token is
//! cached until shortly before it expires and then minted again; there is no
//! refresh token to rotate. `api_token` is the Basic pair (Atlassian) a
//! caller holds at runtime; it needs no exchange, so it is encoded, not
//! minted, but it answers through the same [`MachineGrant::authorization`].
//!
//! A grant is plain data, so a caller that stores credentials itself (a
//! service account's sealed secret) builds one directly and keys the cache by
//! whatever identifies the credential's current rotation.
//!
//! Every exchange goes through [`crate::credential`]'s one HTTP client, so it
//! carries the same timeouts and single retry as a provider credential, and a
//! test points `token_url` at a mock server.
//!
//! `machine_token` mints, or reads from cache, the machine token for a server's
//! connector, resolving its secret references from the bootstrapped secrets
//! document; a cached token is minted again once it nears expiry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod app_jwt;
mod error;

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use systemprompt_models::mcp::deployment::{ConnectorConfig, ConnectorGrant};

pub use app_jwt::{AppJwtClaims, app_jwt_claims, sign_app_jwt};
pub use error::ConnectorGrantError;

use crate::credential::cache::{clamp_ttl, token_for};
use crate::credential::{CredentialError, http};

const INSTALLATION_PLACEHOLDER: &str = "{installation_id}";

/// A machine grant with its secrets resolved, ready to mint.
#[derive(Clone)]
pub enum MachineGrant {
    ClientCredentials {
        token_url: String,
        client_id: String,
        client_secret: String,
        scopes: Vec<String>,
    },
    AppInstallation {
        token_url: String,
        app_id: String,
        private_key_pem: String,
    },
    ApiToken {
        email: String,
        api_token: String,
    },
}

impl std::fmt::Debug for MachineGrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClientCredentials { token_url, .. } => f
                .debug_struct("ClientCredentials")
                .field("token_url", token_url)
                .finish_non_exhaustive(),
            Self::AppInstallation {
                token_url, app_id, ..
            } => f
                .debug_struct("AppInstallation")
                .field("token_url", token_url)
                .field("app_id", app_id)
                .finish_non_exhaustive(),
            Self::ApiToken { .. } => f.debug_struct("ApiToken").finish_non_exhaustive(),
        }
    }
}

impl MachineGrant {
    pub fn resolve<F>(
        server: &str,
        connector: &ConnectorConfig,
        secret: F,
    ) -> Result<Self, ConnectorGrantError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let field = |value: Option<&String>, label: &'static str| {
            value
                .filter(|v| !v.trim().is_empty())
                .cloned()
                .ok_or_else(|| ConnectorGrantError::MissingField {
                    server: server.to_owned(),
                    field: label,
                })
        };
        let lookup = |name: String| {
            secret(&name).filter(|v| !v.is_empty()).ok_or_else(|| {
                ConnectorGrantError::MissingSecret {
                    server: server.to_owned(),
                    secret: name,
                }
            })
        };
        let token_url = field(connector.token_url.as_ref(), "token_url")?;
        match connector.grant {
            ConnectorGrant::AuthorizationCode => Err(ConnectorGrantError::NotMachine {
                server: server.to_owned(),
            }),
            ConnectorGrant::ClientCredentials => Ok(Self::ClientCredentials {
                token_url,
                client_id: lookup(field(
                    connector.client_id_secret.as_ref(),
                    "client_id_secret",
                )?)?,
                client_secret: lookup(field(connector.client_secret.as_ref(), "client_secret")?)?,
                scopes: connector.scopes.clone(),
            }),
            ConnectorGrant::AppInstallation => {
                let installation_id = field(connector.installation_id.as_ref(), "installation_id")?;
                Ok(Self::AppInstallation {
                    token_url: token_url.replace(INSTALLATION_PLACEHOLDER, &installation_id),
                    app_id: field(connector.app_id.as_ref(), "app_id")?,
                    private_key_pem: lookup(field(
                        connector.private_key_secret.as_ref(),
                        "private_key_secret",
                    )?)?,
                })
            },
        }
    }

    pub async fn token(&self, cache_key: &str) -> Result<String, ConnectorGrantError> {
        token_for(cache_key, || async { self.mint().await })
            .await
            .map_err(ConnectorGrantError::Credential)
    }

    pub async fn authorization(&self, cache_key: &str) -> Result<String, ConnectorGrantError> {
        let scheme = match self {
            Self::ApiToken { .. } => "Basic",
            Self::ClientCredentials { .. } | Self::AppInstallation { .. } => "Bearer",
        };
        Ok(format!("{scheme} {}", self.token(cache_key).await?))
    }

    async fn mint(&self) -> Result<(String, Duration), CredentialError> {
        match self {
            Self::ClientCredentials {
                token_url,
                client_id,
                client_secret,
                scopes,
            } => {
                let mut form = vec![
                    ("grant_type", "client_credentials".to_owned()),
                    ("client_id", client_id.clone()),
                    ("client_secret", client_secret.clone()),
                ];
                if !scopes.is_empty() {
                    form.push(("scope", scopes.join(" ")));
                }
                let body = http::post_form(token_url, &form).await?;
                let response: ClientCredentialsResponse =
                    serde_json::from_str(&body).map_err(CredentialError::UnreadableBody)?;
                Ok((response.access_token, clamp_ttl(response.expires_in)))
            },
            Self::AppInstallation {
                token_url,
                app_id,
                private_key_pem,
            } => {
                let jwt = sign_app_jwt(app_id, private_key_pem)?;
                let body = http::post_bearer(token_url, &jwt, GITHUB_HEADERS).await?;
                let response: InstallationTokenResponse =
                    serde_json::from_str(&body).map_err(CredentialError::UnreadableBody)?;
                let remaining = response
                    .expires_at
                    .signed_duration_since(chrono::Utc::now())
                    .num_seconds();
                Ok((
                    response.token,
                    clamp_ttl(Some(remaining.max(1).unsigned_abs())),
                ))
            },
            Self::ApiToken { email, api_token } => Ok((
                STANDARD.encode(format!("{email}:{api_token}")),
                clamp_ttl(None),
            )),
        }
    }
}

pub async fn machine_token(
    server: &str,
    connector: &ConnectorConfig,
) -> Result<String, ConnectorGrantError> {
    let secrets = systemprompt_config::SecretsBootstrap::get()
        .map_err(|e| ConnectorGrantError::Secrets(Box::new(e)))?;
    let grant = MachineGrant::resolve(server, connector, |name| secrets.get(name).cloned())?;
    grant
        .token(&format!("connector:{server}:{}", connector.grant.as_str()))
        .await
}

// Why: GitHub refuses a request with no User-Agent, and pins the REST API
// version by header rather than by path.
const GITHUB_HEADERS: &[(&str, &str)] = &[
    ("Accept", "application/vnd.github+json"),
    ("X-GitHub-Api-Version", "2022-11-28"),
    ("User-Agent", "systemprompt-connector"),
];

#[derive(Deserialize)]
struct ClientCredentialsResponse {
    access_token: String,
    #[serde(default, deserialize_with = "crate::oauth_wire::expires_in")]
    expires_in: Option<u64>,
}

#[derive(Deserialize)]
struct InstallationTokenResponse {
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}
