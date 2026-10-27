//! Unit tests for systemprompt-models crate
//!
//! Tests cover:
//! - A2A protocol models (AgentCard, Task, Message, etc.)
//! - AI service models (AiMessage, AiRequest, MessageRole)
//! - Artifact types (ArtifactType, ChartType, ColumnType, etc.)
//! - Artifact builders (Column, TableHints, DashboardHints, MetricCard, etc.)
//! - API types (PaginationInfo, ApiResponse, CollectionResponse, ApiError,
//!   etc.)
//! - Configuration models (Environment)
//! - Event system models (SystemEventType, A2AEventType)
//! - Authentication models (BaseRoles, AuthRequestError)
//! - Execution models (ExecutionStep, RequestContext, CallSource)
//! - Validators (AgentConfig, AiConfig, Content, Mcp, Skills, RateLimits, Web)

#[cfg(test)]
mod a2a;

#[cfg(test)]
mod enforcement_mode;

#[cfg(test)]
mod subject_source_handle;

#[cfg(test)]
mod a2a_artifact_metadata;

#[cfg(test)]
mod a2a_task_metadata;

#[cfg(test)]
mod task_state_short_names;

#[cfg(test)]
mod plugin_manifest_no_alias;

#[cfg(test)]
mod ai;

#[cfg(test)]
mod api;
#[cfg(test)]
mod api_paths_bridge;
#[cfg(test)]
mod api_responses_specialized;

#[cfg(test)]
mod artifacts;
#[cfg(test)]
mod artifacts_table;

#[cfg(test)]
mod artifacts_extended;

#[cfg(test)]
mod artifacts_media;

#[cfg(test)]
mod ai_tool_call;

#[cfg(test)]
mod ai_tool_result_formatter;

#[cfg(test)]
mod ai_tool_result_formatter_extended;

#[cfg(test)]
mod message_artifact;

#[cfg(test)]
#[cfg(test)]
mod events;

#[cfg(test)]
mod auth;

#[cfg(test)]
mod execution;
#[cfg(test)]
mod execution_context;

#[cfg(test)]
mod validators;

#[cfg(test)]
mod profile;


#[cfg(test)]
mod profile_secrets_vault;

#[cfg(test)]
mod profile_secrets_refresh;

#[cfg(test)]
mod manifest_config_provider;

#[cfg(test)]
mod desktop_policy_validation;

#[cfg(test)]
mod marketplace_mirror_wire;

#[cfg(test)]
mod profile_services_sources;

#[cfg(test)]
mod profile_audit;
#[cfg(test)]
mod profile_database_migrate_on_boot;
#[cfg(test)]
mod profile_instance_identity;
#[cfg(test)]
mod profile_judge;
#[cfg(test)]
mod profile_observability;
#[cfg(test)]
mod profile_server_capacity;
#[cfg(test)]
mod profile_server_shutdown;

#[cfg(test)]
mod oci_reference;

#[cfg(test)]
mod services_bundle;


#[cfg(test)]
mod wire_codec;

#[cfg(test)]
mod wire_inspect;

#[cfg(test)]
mod wire_origin;

#[cfg(test)]
mod agui;
#[cfg(test)]
mod agui_events;

#[cfg(test)]
mod ai_tool_model_config;

#[cfg(test)]
mod text_utils;

#[cfg(test)]
mod mime;

#[cfg(test)]
mod routing;

#[cfg(test)]
mod marketplace;
#[cfg(test)]
mod marketplace_opencode_themes;


#[cfg(test)]
mod gateway_hash;

#[cfg(test)]
mod config_postgres;

#[cfg(test)]
mod permission;

#[cfg(test)]
mod mcp_client_profile;
#[cfg(test)]
mod mcp_tool_result_metadata;

#[cfg(test)]
mod secrets;

#[cfg(test)]
mod net;

#[cfg(test)]
#[cfg(test)]
mod subprocess_deployment_host;

#[cfg(test)]
mod services_hooks;

#[cfg(test)]
mod services_other;
#[cfg(test)]
mod services_teams;

#[cfg(test)]
mod content_config;

#[cfg(test)]
mod services_plugin;
#[cfg(test)]
mod services_plugin_dependencies;

#[cfg(test)]
mod auth_enums;

#[cfg(test)]
mod ai_content_part;

#[cfg(test)]
mod services_agent;

#[cfg(test)]
mod disk_agent_config;

#[cfg(test)]
mod api_error_codes;

#[cfg(test)]
mod execution_plan;

#[cfg(test)]
mod mcp_capabilities;

#[cfg(test)]
mod auth_permission;

#[cfg(test)]
mod auth_types;

#[cfg(test)]
mod bridge_host_kind;

#[cfg(test)]
mod bridge_ids;

#[cfg(test)]
mod bridge_manifest_version;

#[cfg(test)]
mod mcp_deployment;

#[cfg(test)]
mod mcp_connector_grant;
#[cfg(test)]
mod mcp_deployment_connector;

#[cfg(test)]
mod errors_models;

#[cfg(test)]
mod env_interpolation;

#[cfg(test)]
mod domain_config;

#[cfg(test)]
mod users_summary;

#[cfg(test)]
mod validation_report;

#[cfg(test)]
mod oauth_models;

#[cfg(test)]
mod modules_tests;

#[cfg(test)]
mod ai_sampling;

#[cfg(test)]
mod ai_media_types;

#[cfg(test)]
#[cfg(test)]
mod config_rate_limits;

#[cfg(test)]
mod content_models;

#[cfg(test)]
mod services_includable;


#[cfg(test)]
mod schema_gemini_invariants;
#[cfg(test)]
mod schema_sanitizer;

#[cfg(test)]
mod validators_driven;

#[cfg(test)]
mod wire_cache_convention;

#[cfg(test)]
mod wire_terminal_discipline;

#[cfg(test)]
mod wire_usage_omission;

#[cfg(test)]
mod wire_streaming;

#[cfg(test)]
mod wire_sse;

#[cfg(test)]
mod wire_stream_abort;

#[cfg(test)]
mod wire_stream_usage_order;

#[cfg(test)]
mod profile_config_planes;
#[cfg(test)]
mod profile_layering;
#[cfg(test)]
mod profile_unapplied_planes;
#[cfg(test)]
mod profile_validation;

#[cfg(test)]
mod profile_database_scaling;

#[cfg(test)]
mod profile_storage_gcs;

#[cfg(test)]
mod profile_storage_s3;

#[cfg(test)]
mod auth_claims;

#[cfg(test)]
mod canonical_request;

#[cfg(test)]
mod ai_template_validation;

#[cfg(test)]
mod events_builders;

#[cfg(test)]
mod managed_bundle;

#[cfg(test)]
mod mcp_apps;

#[cfg(test)]
mod services_port_offset;

#[cfg(test)]
mod services_validation;

#[cfg(test)]
mod artifacts_provenance;

#[cfg(test)]
mod events_system;

#[cfg(test)]
mod models_misc_edges;

#[cfg(test)]
mod ai_request_response;

#[cfg(test)]
mod extension_manifest;


#[cfg(test)]
mod events_context_summary;

#[cfg(test)]
mod artifacts_message;

#[cfg(test)]
mod services_rules;

#[cfg(test)]
mod feedback_contracts;

#[cfg(test)]
mod skill_frontmatter_render;

#[cfg(test)]
mod documents_format;

#[cfg(test)]
mod documents_projection;

#[cfg(test)]
mod documents_edit_op;

#[cfg(test)]
mod documents_access;

#[cfg(test)]
mod documents_uri;

#[cfg(test)]
mod documents_drive;
