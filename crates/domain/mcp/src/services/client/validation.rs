//! Connection validation for MCP servers.
//!
//! Probes a server over the streamable-HTTP transport to confirm it speaks the
//! MCP protocol and exposes tools, producing a [`McpConnectionResult`]. Covers
//! the OAuth-gated case (port reachability only) and internal/external URL
//! rewriting for loopback access.

use crate::error::McpDomainResult;
use rmcp::ServiceExt;
use rmcp::model::{ClientCapabilities, ClientInfo, Implementation};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use std::time::Duration;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_models::execution::context::RequestContext;
use tokio::time::timeout;

use super::HttpClientWithContext;
use super::types::{McpConnectionResult, McpProtocolInfo, ValidationResult};

pub async fn validate_connection(
    service_name: &str,
    host: &str,
    port: u16,
) -> McpDomainResult<McpConnectionResult> {
    let url = format!("http://{host}:{port}/mcp");
    validate_connection_by_url(service_name, &url).await
}

pub async fn validate_connection_with_auth(
    service_name: &str,
    host: &str,
    port: u16,
    requires_oauth: bool,
) -> McpDomainResult<McpConnectionResult> {
    if requires_oauth {
        Ok(validate_oauth_service(service_name, host, port))
    } else {
        validate_connection(service_name, host, port).await
    }
}

pub async fn validate_connection_by_url(
    service_name: &str,
    url: &str,
) -> McpDomainResult<McpConnectionResult> {
    let connection_start = std::time::Instant::now();

    let connection_result = timeout(
        Duration::from_secs(15),
        connect_and_validate(url, service_name),
    )
    .await;

    let connection_time = connection_start.elapsed().as_millis() as u32;

    match connection_result {
        Ok(Ok((server_info, validation_result))) => Ok(McpConnectionResult {
            service_name: service_name.to_owned(),
            success: validation_result.success,
            error_message: validation_result.error_message,
            connection_time_ms: connection_time,
            server_info: Some(server_info),
            tools_count: validation_result.tools_count,
            validation_type: validation_result.validation_type,
        }),
        Ok(Err(e)) => Ok(McpConnectionResult {
            service_name: service_name.to_owned(),
            success: false,
            error_message: Some(e.to_string()),
            connection_time_ms: connection_time,
            server_info: None,
            tools_count: 0,
            validation_type: "connection_failed".to_owned(),
        }),
        Err(_) => Ok(McpConnectionResult {
            service_name: service_name.to_owned(),
            success: false,
            error_message: Some("Connection timeout".to_owned()),
            connection_time_ms: connection_time,
            server_info: None,
            tools_count: 0,
            validation_type: "timeout".to_owned(),
        }),
    }
}

fn validate_oauth_service(service_name: &str, host: &str, port: u16) -> McpConnectionResult {
    let connection_start = std::time::Instant::now();

    let port_check = std::net::TcpStream::connect(format!("{host}:{port}"));
    let connection_time = connection_start.elapsed().as_millis() as u32;

    match port_check {
        Ok(_) => McpConnectionResult {
            service_name: service_name.to_owned(),
            success: true,
            error_message: None,
            connection_time_ms: connection_time,
            server_info: Some(McpProtocolInfo {
                server_name: service_name.to_owned(),
                version: "unknown".to_owned(),
                protocol_version: "unknown".to_owned(),
            }),
            tools_count: 0,
            validation_type: "auth_required".to_owned(),
        },
        Err(e) => McpConnectionResult {
            service_name: service_name.to_owned(),
            success: false,
            error_message: Some(format!("Port not responding: {e}")),
            connection_time_ms: connection_time,
            server_info: None,
            tools_count: 0,
            validation_type: "port_unavailable".to_owned(),
        },
    }
}

async fn connect_and_validate(
    url: &str,
    service_name: &str,
) -> McpDomainResult<(McpProtocolInfo, ValidationResult)> {
    let context = RequestContext::new(
        SessionId::new(format!("mcp-validate-{service_name}")),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::system(),
    );
    let config = StreamableHttpClientTransportConfig::with_uri(url);
    let transport =
        StreamableHttpClientTransport::with_client(HttpClientWithContext::new(context), config);

    let client_info = ClientInfo::new(
        ClientCapabilities::default(),
        Implementation::new(
            format!("systemprompt.io MCP Validator for {service_name}"),
            "1.0.0",
        ),
    );

    let client = client_info.serve(transport).await?;

    let peer_info = client.peer_info().ok_or_else(|| {
        crate::error::McpDomainError::Internal("Failed to get peer info from MCP client".to_owned())
    })?;

    let server_info = McpProtocolInfo {
        server_name: if peer_info.server_info.name.is_empty() {
            service_name.to_owned()
        } else {
            peer_info.server_info.name.clone()
        },
        version: if peer_info.server_info.version.is_empty() {
            "1.0.0".to_owned()
        } else {
            peer_info.server_info.version.clone()
        },
        protocol_version: peer_info.protocol_version.to_string(),
    };

    let validation_result = match client.list_tools(None).await {
        Ok(tools_response) => {
            let tools_count = tools_response.tools.len();

            if tools_count > 0 {
                ValidationResult {
                    success: true,
                    error_message: None,
                    tools_count,
                    validation_type: "mcp_validated".to_owned(),
                }
            } else {
                ValidationResult {
                    success: false,
                    error_message: Some(
                        "No tools returned - service may require authentication".to_owned(),
                    ),
                    tools_count: 0,
                    validation_type: "no_tools".to_owned(),
                }
            }
        },
        Err(e) => ValidationResult {
            success: false,
            error_message: Some(format!("Tools request failed: {e}")),
            tools_count: 0,
            validation_type: "tools_request_failed".to_owned(),
        },
    };

    client.cancel().await?;
    Ok((server_info, validation_result))
}

pub fn rewrite_url_for_internal_use(url: &str) -> String {
    use systemprompt_models::Config;

    let Ok(config) = Config::get() else {
        return url.to_owned();
    };
    let external_url = &config.api_external_url;
    let internal_url = &config.api_server_url;

    if url.starts_with(external_url) {
        url.replace(external_url, internal_url)
    } else {
        url.to_owned()
    }
}
