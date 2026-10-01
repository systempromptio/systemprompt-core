//! CLI value parsers for fail-fast validation at command line boundaries.
//!
//! Each parser is a clap `value_parser`: a malformed identifier is rejected
//! while the arguments are parsed, so it surfaces as a usage error instead of
//! reaching a command body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::error::IdValidationError;
use systemprompt_identifiers::{
    AgentName, AiRequestId, CampaignId, CategoryId, Email, JobName, LinkClickId, LinkId,
    McpServerId, McpToolName, ModelId, PluginId, ProfileName, ProviderId, SecretName, ServiceName,
    TenantId, TraceId,
};

macro_rules! id_parsers {
    ($($fn_name:ident => $ty:ident),* $(,)?) => {
        $(
            pub fn $fn_name(s: &str) -> Result<$ty, IdValidationError> {
                $ty::try_new(s)
            }
        )*
    };
}

id_parsers! {
    parse_profile_name => ProfileName,
    parse_email => Email,
    parse_agent_name => AgentName,
    parse_ai_request_id => AiRequestId,
    parse_campaign_id => CampaignId,
    parse_category_id => CategoryId,
    parse_job_name => JobName,
    parse_link_click_id => LinkClickId,
    parse_link_id => LinkId,
    parse_mcp_server_id => McpServerId,
    parse_mcp_tool_name => McpToolName,
    parse_plugin_id => PluginId,
    parse_service_name => ServiceName,
    parse_tenant_id => TenantId,
    parse_trace_id => TraceId,
    parse_provider_id => ProviderId,
    parse_model_id => ModelId,
    parse_secret_name => SecretName,
}
