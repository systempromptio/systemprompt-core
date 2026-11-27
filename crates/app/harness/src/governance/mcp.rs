//! Managed MCP servers as the engine connects to them.
//!
//! A server's headers are read again before every request: the declared ones,
//! plus the user's JWT as `Authorization` only for a server the projection
//! marked for it and that sits on the instance's own origin. A JWT that cannot
//! be had, or has expired, is left out rather than sent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use chrono::{TimeDelta, Utc};
use systemprompt_engine::run::McpSpec;
use systemprompt_engine::mcp::{BoxError, HeaderSource};
use systemprompt_device::client::{CredentialSource, Freshness};
use crate::projection::ManagedMcp;
use sp_protocol::origin::bearer_allowed;
use url::Url;

use crate::governance::instance::JWT_MIN_VALIDITY;

pub(crate) fn specs(
    servers: &[ManagedMcp],
    instance: &Url,
    credentials: &Arc<dyn CredentialSource>,
) -> Vec<McpSpec> {
    servers
        .iter()
        .map(|server| spec(server, instance, credentials))
        .collect()
}

fn spec(server: &ManagedMcp, instance: &Url, credentials: &Arc<dyn CredentialSource>) -> McpSpec {
    let bearer =
        server.bearer && Url::parse(&server.url).is_ok_and(|url| bearer_allowed(&url, instance));
    let declared = server
        .headers
        .iter()
        .filter(|(name, _)| !name.eq_ignore_ascii_case("authorization"))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    let source = InstanceHeaders {
        server: server.name.clone(),
        declared,
        bearer,
        credentials: Arc::clone(credentials),
    };
    McpSpec {
        name: server.name.clone(),
        url: server.url.clone(),
        headers: Arc::new(source),
    }
}

struct InstanceHeaders {
    server: String,
    declared: Vec<(String, String)>,
    bearer: bool,
    credentials: Arc<dyn CredentialSource>,
}

impl fmt::Debug for InstanceHeaders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self
            .declared
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        f.debug_struct("InstanceHeaders")
            .field("server", &self.server)
            .field("declared", &names)
            .field("bearer", &self.bearer)
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl HeaderSource for InstanceHeaders {
    async fn headers(&self) -> Result<Vec<(String, String)>, BoxError> {
        let mut headers = self.declared.clone();
        if !self.bearer {
            return Ok(headers);
        }
        match self
            .credentials
            .jwt(Freshness::ValidFor(JWT_MIN_VALIDITY))
            .await
        {
            Ok(jwt) if !jwt.is_expired(Utc::now(), TimeDelta::zero()) => {
                headers.push((
                    "Authorization".to_owned(),
                    format!("Bearer {}", jwt.expose()),
                ));
            },
            Ok(_) => {
                tracing::debug!(
                    server = %self.server,
                    "the session token has expired; MCP request sent without one"
                );
            },
            Err(error) => {
                tracing::debug!(
                    server = %self.server,
                    error = %error,
                    "no session token; MCP request sent without one"
                );
            },
        }
        Ok(headers)
    }
}
