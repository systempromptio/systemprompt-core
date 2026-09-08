//! External MCP sessions are bound to a caller and their current provider
//! grant.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use sha2::{Digest, Sha256};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_mcp::repository::{ExternalSessionBinding, McpProxyIdentityRepository};
use systemprompt_models::RequestContext;

use super::super::backend::ProxyError;

pub(super) struct SessionGuard<'a> {
    repository: &'a McpProxyIdentityRepository,
    server: &'a str,
    user_id: &'a UserId,
    credential_hash: [u8; 32],
}

impl<'a> SessionGuard<'a> {
    pub(super) fn new(
        repository: &'a McpProxyIdentityRepository,
        server: &'a str,
        context: &'a RequestContext,
        headers: &HashMap<HeaderName, HeaderValue>,
    ) -> Self {
        let mut headers: Vec<_> = headers.iter().collect();
        headers.sort_unstable_by_key(|(name, _)| name.as_str());
        let mut hash = Sha256::new();
        for (name, value) in headers {
            hash.update((name.as_str().len() as u64).to_be_bytes());
            hash.update(name.as_str().as_bytes());
            hash.update((value.as_bytes().len() as u64).to_be_bytes());
            hash.update(value.as_bytes());
        }
        Self {
            repository,
            server,
            user_id: context.user_id(),
            credential_hash: hash.finalize().into(),
        }
    }

    const fn binding<'b>(&'b self, session_id: &'b SessionId) -> ExternalSessionBinding<'b> {
        ExternalSessionBinding {
            server: self.server,
            session_id,
            user_id: self.user_id,
            credential_hash: &self.credential_hash,
        }
    }

    fn failed(&self, error: &systemprompt_mcp::McpDomainError) -> ProxyError {
        tracing::warn!(%error, server = self.server, "External MCP session persistence failed");
        ProxyError::Forbidden {
            service: self.server.to_owned(),
        }
    }

    pub(super) async fn accepts(&self, session: &HeaderValue) -> Result<bool, ProxyError> {
        let Ok(session) = session.to_str() else {
            return Ok(false);
        };
        self.repository
            .accepts_external(&self.binding(&SessionId::new(session)))
            .await
            .map_err(|error| self.failed(&error))
    }

    pub(super) async fn remember(&self, headers: &HeaderMap) -> Result<(), ProxyError> {
        if let Some(session) = headers.get("mcp-session-id") {
            let session = session.to_str().map_err(|error| {
                tracing::warn!(%error, server = self.server, "Invalid external MCP session header");
                ProxyError::Forbidden {
                    service: self.server.to_owned(),
                }
            })?;
            let remembered = self
                .repository
                .remember_external(&self.binding(&SessionId::new(session)))
                .await
                .map_err(|error| self.failed(&error))?;
            if !remembered {
                return Err(ProxyError::Forbidden {
                    service: self.server.to_owned(),
                });
            }
        }
        Ok(())
    }

    pub(super) async fn forget(&self, headers: &HeaderMap) -> Result<(), ProxyError> {
        if let Some(session) = headers
            .get("mcp-session-id")
            .and_then(|value| value.to_str().ok())
        {
            self.repository
                .forget_external(&self.binding(&SessionId::new(session)))
                .await
                .map_err(|error| self.failed(&error))?;
        }
        Ok(())
    }
}
