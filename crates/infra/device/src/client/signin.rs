//! Sign-in: discovery, the health probe, the device-link page, the
//! one-time-code exchange and PAT to JWT sessions.
//!
//! An instance without a discovery document (`404`, older than 0.76) is reached
//! through the fallback device-link path. A probe additionally tells a
//! systemprompt instance from any other address: without a discovery document
//! the sign-in capabilities route must answer. Paths the discovery document
//! names are resolved on the instance's origin and refused when they would
//! leave it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};
use reqwest::header::CONTENT_TYPE;
use sp_protocol::ids::SessionId;
use sp_protocol::origin::{is_device_link_redirect, resolve_instance_path};
use sp_protocol::wire::auth::{
    Capabilities, Health, PatSession, SessionPatRequest, SessionPatResponse,
};
use sp_protocol::wire::discovery::Discovery;
use sp_protocol::wire::{headers, paths};
use sp_protocol::{Jwt, OneTimeCode, Pat};
use url::Url;

use crate::client::client::SpClient;
use crate::client::error::{ClientError, Incompatibility, excerpt};
use crate::client::http::{DEFAULT_TIMEOUT, SMALL_BODY, decode, exchange, failure, protocol};

const DISCOVERY_ENDPOINT: &str = "GET /.well-known/systemprompt-bridge";
const CAPABILITIES_ENDPOINT: &str = "GET /v1/auth/bridge/capabilities";
const HEALTH_ENDPOINT: &str = "GET /api/v1/health";
const SESSION_PAT_ENDPOINT: &str = "POST /v1/auth/bridge/session-pat";
const PAT_ENDPOINT: &str = "POST /v1/auth/bridge/pat";

const HEALTH_TIMEOUT: Duration = Duration::from_secs(10);

/// Where an instance's device-link page is and how it presents itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceDiscovery {
    pub discovered: bool,
    pub device_link: Url,
    pub display_name: Option<String>,
    pub sign_in_hint: Option<String>,
    pub enabled_hosts: Vec<String>,
}

impl InstanceDiscovery {
    pub fn device_link_url(&self, redirect: Option<&Url>) -> Result<Url, ClientError> {
        let mut page = self.device_link.clone();
        page.set_query(None);
        page.set_fragment(None);
        if let Some(redirect) = redirect {
            if !is_device_link_redirect(redirect)
                || !redirect.username().is_empty()
                || redirect.password().is_some()
            {
                return Err(ClientError::InvalidInput {
                    what: "the redirect must be http://127.0.0.1:<port> or http://localhost:<port>",
                });
            }
            page.query_pairs_mut()
                .append_pair("redirect", redirect.as_str());
        }
        Ok(page)
    }
}

/// What a probe learned about an address that is a systemprompt instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceProbe {
    pub discovery: InstanceDiscovery,
    pub capabilities: Capabilities,
    pub version: Option<String>,
}

/// A JWT minted from a PAT, with the session the instance bound it to and the
/// device token it issued beside it.
#[derive(Debug, Clone)]
pub struct JwtSession {
    pub jwt: Jwt,
    pub ttl: Duration,
    pub expires_at: DateTime<Utc>,
    pub session_id: Option<SessionId>,
    pub device_token: Option<Jwt>,
}

impl SpClient {
    pub async fn discover(&self) -> Result<InstanceDiscovery, ClientError> {
        self.discovery_document().await?.map_or_else(
            || self.fallback_discovery(),
            |document| self.discovered(document),
        )
    }

    pub async fn probe(&self) -> Result<InstanceProbe, ClientError> {
        let (document, capabilities, health) = tokio::join!(
            self.discovery_document(),
            self.capabilities_answer(),
            self.health()
        );
        let document = match document {
            Ok(found) => found,
            Err(ClientError::Protocol { .. }) => None,
            Err(error) => return Err(error),
        };
        let capabilities = match capabilities {
            Ok(found) => found,
            Err(ClientError::Protocol { .. }) => None,
            Err(error) => return Err(error),
        };
        let discovery = match document {
            Some(document) => self.discovered(document)?,
            None if capabilities.is_some() => self.fallback_discovery()?,
            None => {
                return Err(ClientError::Incompatible {
                    endpoint: DISCOVERY_ENDPOINT,
                    what: Incompatibility::NotAnInstance,
                });
            },
        };
        Ok(InstanceProbe {
            discovery,
            capabilities: capabilities.unwrap_or_default(),
            version: health.and_then(|health| health.version),
        })
    }

    pub async fn capabilities(&self) -> Result<Capabilities, ClientError> {
        Ok(self.capabilities_answer().await?.unwrap_or_default())
    }

    pub async fn exchange_code(
        &self,
        code: &OneTimeCode,
        device_name: Option<&str>,
    ) -> Result<Pat, ClientError> {
        let body = SessionPatRequest {
            code: code.expose(),
            device_name,
        };
        let request = self.post(self.at(paths::SESSION_PAT)?).json(&body);
        let answer = exchange(SESSION_PAT_ENDPOINT, request, DEFAULT_TIMEOUT, SMALL_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(SESSION_PAT_ENDPOINT, &answer));
        }
        let response: SessionPatResponse = decode(SESSION_PAT_ENDPOINT, &answer)?;
        Ok(response.pat)
    }

    pub async fn mint_session(
        &self,
        pat: &Pat,
        session_id: Option<&SessionId>,
    ) -> Result<JwtSession, ClientError> {
        let mut request = self
            .post(self.at(paths::PAT)?)
            .bearer_auth(pat.expose())
            .header(CONTENT_TYPE, "application/json")
            .body("{}");
        if let Some(session) = session_id
            .map(SessionId::as_str)
            .filter(|session| is_header_safe(session))
        {
            request = request.header(headers::SESSION_ID, session);
        }
        let answer = exchange(PAT_ENDPOINT, request, DEFAULT_TIMEOUT, SMALL_BODY).await?;
        if answer.status.as_u16() != 200 {
            return Err(failure(PAT_ENDPOINT, &answer));
        }
        let session: PatSession = decode(PAT_ENDPOINT, &answer)?;
        let session_id = session.session_id();
        let now = Utc::now();
        let expires_at = session
            .token
            .expires_at()
            .unwrap_or_else(|| after(now, session.ttl));
        Ok(JwtSession {
            ttl: Duration::from_secs(session.ttl),
            expires_at,
            session_id,
            device_token: session.device_token,
            jwt: session.token,
        })
    }

    async fn discovery_document(&self) -> Result<Option<Discovery>, ClientError> {
        let request = self.get(self.at(paths::DISCOVERY)?);
        let answer = exchange(DISCOVERY_ENDPOINT, request, DEFAULT_TIMEOUT, SMALL_BODY).await?;
        match answer.status.as_u16() {
            404 => Ok(None),
            200 => {
                let document: Discovery = decode(DISCOVERY_ENDPOINT, &answer)?;
                if !document.is_compatible() {
                    return Err(ClientError::Incompatible {
                        endpoint: DISCOVERY_ENDPOINT,
                        what: Incompatibility::DiscoveryContract {
                            found: excerpt(&document.contract).unwrap_or_default(),
                        },
                    });
                }
                Ok(Some(document))
            },
            _ => Err(failure(DISCOVERY_ENDPOINT, &answer)),
        }
    }

    async fn capabilities_answer(&self) -> Result<Option<Capabilities>, ClientError> {
        let request = self.get(self.at(paths::CAPABILITIES)?);
        let answer = exchange(CAPABILITIES_ENDPOINT, request, DEFAULT_TIMEOUT, SMALL_BODY).await?;
        match answer.status.as_u16() {
            404 => Ok(None),
            200 => decode(CAPABILITIES_ENDPOINT, &answer).map(Some),
            _ => Err(failure(CAPABILITIES_ENDPOINT, &answer)),
        }
    }

    async fn health(&self) -> Option<Health> {
        let request = self.get(self.at(paths::HEALTH).ok()?);
        let answer = exchange(HEALTH_ENDPOINT, request, HEALTH_TIMEOUT, SMALL_BODY)
            .await
            .ok()?;
        if answer.status.as_u16() != 200 {
            return None;
        }
        decode(HEALTH_ENDPOINT, &answer).ok()
    }

    fn discovered(&self, document: Discovery) -> Result<InstanceDiscovery, ClientError> {
        let device_link =
            resolve_instance_path(self.base(), &document.device_link_path).map_err(|_foreign| {
                protocol(
                    DISCOVERY_ENDPOINT,
                    Some(200),
                    "the device-link path leaves the instance",
                )
            })?;
        let branding = document.branding.unwrap_or_default();
        Ok(InstanceDiscovery {
            discovered: true,
            device_link,
            display_name: branding.display_name,
            sign_in_hint: branding.sign_in_hint,
            enabled_hosts: document.enabled_hosts,
        })
    }

    fn fallback_discovery(&self) -> Result<InstanceDiscovery, ClientError> {
        Ok(InstanceDiscovery {
            discovered: false,
            device_link: self.at(paths::FALLBACK_DEVICE_LINK)?,
            display_name: None,
            sign_in_hint: None,
            enabled_hosts: Vec::new(),
        })
    }
}

fn after(now: DateTime<Utc>, ttl_secs: u64) -> DateTime<Utc> {
    TimeDelta::try_seconds(i64::try_from(ttl_secs).unwrap_or(i64::MAX))
        .and_then(|ttl| now.checked_add_signed(ttl))
        .unwrap_or(now)
}

fn is_header_safe(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_graphic())
}
