//! Output builders for `plugins mcp validate`: the per-server result card for a
//! failed check and for a completed connection probe.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ServiceName;
use systemprompt_mcp::services::client::McpConnectionResult;

use super::types::{McpServerInfo, McpValidateOutput};

#[derive(Debug)]
pub struct FailureDetail {
    pub health_status: &'static str,
    pub validation_type: &'static str,
    pub latency_ms: u32,
    pub issue: String,
    pub message: String,
}

pub fn failure_output(service_name: &ServiceName, detail: FailureDetail) -> McpValidateOutput {
    McpValidateOutput {
        server: service_name.clone(),
        valid: false,
        health_status: detail.health_status.to_owned(),
        validation_type: detail.validation_type.to_owned(),
        tools_count: None,
        latency_ms: detail.latency_ms,
        server_info: None,
        issues: vec![detail.issue],
        message: detail.message,
    }
}

pub fn success_output(
    service_name: &ServiceName,
    validation_result: McpConnectionResult,
) -> McpValidateOutput {
    let health_status = validation_result.health_status().to_owned();
    let message = validation_result.status_description();

    let server_info = validation_result.server_info.map(|info| McpServerInfo {
        name: info.server_name,
        version: info.version,
        protocol_version: info.protocol_version,
    });

    let issues = validation_result
        .error_message
        .as_ref()
        .filter(|e| !e.is_empty())
        .map_or_else(Vec::new, |e| vec![e.clone()]);

    McpValidateOutput {
        server: service_name.clone(),
        valid: validation_result.success,
        health_status,
        validation_type: validation_result.validation_type,
        tools_count: validation_result.tools_count,
        latency_ms: validation_result.connection_time_ms,
        server_info,
        issues,
        message,
    }
}
