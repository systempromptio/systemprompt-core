//! The MCP handshake a kernel health check performs against a hosted server.
//!
//! It posts `initialize` with a hop token minted for the server by the health
//! principal, then (bound to the session the server opened) `notifications/
//! initialized` and `tools/list`, and closes the session. A listener that
//! does not answer the handshake with an MCP `initialize` result within the
//! deadline is unhealthy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use serde_json::{Value as JsonValue, json};
use systemprompt_client::{GuardedClient, GuardedClientConfig};
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, McpServerId, ServiceName, SessionId, TraceId, UserId, headers,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_traits::ContextPropagation;

use super::types::{McpConnectionResult, McpProtocolInfo};
use crate::services::health_probe::{HEALTH_PRINCIPAL, HealthHop};

const HANDSHAKE_DEADLINE: Duration = Duration::from_secs(15);
const SESSION_HEADER: &str = "mcp-session-id";

#[derive(Debug, thiserror::Error)]
enum HandshakeError {
    #[error("the health hop could not be minted")]
    Hop(#[source] jsonwebtoken::errors::Error),
    #[error("the health client could not be built")]
    Client(#[source] reqwest::Error),
    #[error("the health probe address was refused")]
    Url(#[source] systemprompt_models::net::OutboundUrlError),
    #[error("the request failed")]
    Request(#[source] reqwest::Error),
    #[error("the server answered {0}")]
    Status(reqwest::StatusCode),
    #[error("the server did not answer with an MCP {0} result")]
    NotMcp(&'static str),
    #[error("the handshake did not finish within {}s", HANDSHAKE_DEADLINE.as_secs())]
    Timeout,
}

struct Handshake<'a> {
    client: GuardedClient,
    url: String,
    server: McpServerId,
    hop: &'a HealthHop,
    context: RequestContext,
}

impl Handshake<'_> {
    // JSON: MCP JSON-RPC frames of the handshake, posted and answered verbatim.
    async fn post(
        &self,
        body: &JsonValue,
        session: Option<&str>,
    ) -> Result<(Option<String>, Option<JsonValue>), HandshakeError> {
        let hop = self
            .hop
            .mint(&self.server, session)
            .map_err(HandshakeError::Hop)?;
        let mut request = self
            .client
            .post(&self.url)
            .map_err(HandshakeError::Url)?
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header(headers::MCP_HOP, hop)
            .json(body);
        for (name, value) in &self.context.to_headers() {
            request = request.header(name, value);
        }
        if let Some(session) = session {
            request = request.header(SESSION_HEADER, session);
        }
        let response = request.send().await.map_err(HandshakeError::Request)?;
        if !response.status().is_success() {
            return Err(HandshakeError::Status(response.status()));
        }
        let opened = response
            .headers()
            .get(SESSION_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let text = response.text().await.map_err(HandshakeError::Request)?;
        Ok((opened, frame(&text)))
    }

    async fn close(&self, session: &str) {
        let Ok(hop) = self.hop.mint(&self.server, Some(session)) else {
            return;
        };
        let Ok(request) = self.client.request(reqwest::Method::DELETE, &self.url) else {
            return;
        };
        let mut request = request
            .header(headers::MCP_HOP, hop)
            .header(SESSION_HEADER, session);
        for (name, value) in &self.context.to_headers() {
            request = request.header(name, value);
        }
        if let Err(error) = request.send().await {
            tracing::debug!(server = %self.server, %error, "health session close failed");
        }
    }
}

// JSON: the MCP JSON-RPC frame of a plain or event-stream response body.
fn frame(text: &str) -> Option<JsonValue> {
    serde_json::from_str(text).ok().or_else(|| {
        text.lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .find_map(|data| serde_json::from_str(data.trim()).ok())
    })
}

async fn run(
    handshake: &Handshake<'_>,
) -> Result<(McpProtocolInfo, Option<usize>), HandshakeError> {
    let (session, initialized) = handshake
        .post(
            &json!({
                "jsonrpc": "2.0", "id": 0, "method": "initialize",
                "params": {
                    "protocolVersion": crate::mcp_protocol_version_str(),
                    "capabilities": {},
                    "clientInfo": {"name": "systemprompt.io MCP health", "version": "1.0.0"}
                }
            }),
            None,
        )
        .await?;
    let result = initialized
        .as_ref()
        .and_then(|frame| frame.get("result"))
        .filter(|result| {
            result.get("serverInfo").is_some() || result.get("protocolVersion").is_some()
        })
        .ok_or(HandshakeError::NotMcp("initialize"))?;
    let info = McpProtocolInfo {
        implementation_name: result["serverInfo"]["name"]
            .as_str()
            .unwrap_or(handshake.server.as_str())
            .to_owned(),
        version: result["serverInfo"]["version"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned(),
        protocol_version: result["protocolVersion"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned(),
    };
    let Some(session) = session else {
        return Ok((info, None));
    };
    let tools = list_tools(handshake, &session).await;
    handshake.close(&session).await;
    Ok((info, tools))
}

async fn list_tools(handshake: &Handshake<'_>, session: &str) -> Option<usize> {
    handshake
        .post(
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            Some(session),
        )
        .await
        .ok()?;
    let (_, listed) = handshake
        .post(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
            Some(session),
        )
        .await
        .ok()?;
    listed?
        .get("result")?
        .get("tools")?
        .as_array()
        .map(Vec::len)
}

pub(super) async fn validate_hosted_handshake(
    service_name: &ServiceName,
    host: &str,
    port: u16,
    hop: &HealthHop,
) -> McpConnectionResult {
    let started = Instant::now();
    let outcome = match GuardedClient::configured(
        &GuardedClientConfig::for_configured().with_timeout(HANDSHAKE_DEADLINE),
    ) {
        Ok(client) => {
            let handshake = Handshake {
                client,
                url: format!("http://{host}:{port}/mcp"),
                server: McpServerId::new(service_name.as_str()),
                hop,
                context: RequestContext::new(
                    SessionId::new(HEALTH_PRINCIPAL),
                    TraceId::generate(),
                    ContextId::derived_from_mcp_validation(service_name),
                    AgentName::system(),
                    Actor::anonymous(UserId::new(HEALTH_PRINCIPAL)),
                ),
            };
            tokio::time::timeout(HANDSHAKE_DEADLINE, run(&handshake))
                .await
                .unwrap_or(Err(HandshakeError::Timeout))
        },
        Err(source) => Err(HandshakeError::Client(source)),
    };
    let connection_time_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
    match outcome {
        Ok((info, tools_count)) => McpConnectionResult {
            service_name: service_name.clone(),
            success: true,
            error_message: None,
            connection_time_ms,
            server_info: Some(info),
            tools_count,
            validation_type: "mcp_validated".to_owned(),
        },
        Err(error) => McpConnectionResult {
            service_name: service_name.clone(),
            success: false,
            validation_type: match error {
                HandshakeError::Timeout => "timeout",
                _ => "connection_failed",
            }
            .to_owned(),
            error_message: Some(error.to_string()),
            connection_time_ms,
            server_info: None,
            tools_count: None,
        },
    }
}
