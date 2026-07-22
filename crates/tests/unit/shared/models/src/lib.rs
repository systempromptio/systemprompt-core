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
//! - Authentication models (BaseRoles, AuthError)
//! - Execution models (ExecutionStep, RequestContext, CallSource)
//! - Validators (AgentConfig, AiConfig, Content, Mcp, Skills, RateLimits, Web)

#[cfg(test)]
mod a2a;

#[cfg(test)]
mod a2a_artifact_metadata;

#[cfg(test)]
mod a2a_task_metadata;

#[cfg(test)]
mod ai;

#[cfg(test)]
mod api;

#[cfg(test)]
mod artifacts;

#[cfg(test)]
mod artifacts_extended;

#[cfg(test)]
mod artifacts_media;

#[cfg(test)]
mod ai_tool_call;

#[cfg(test)]
mod message_artifact;

#[cfg(test)]
mod config;

#[cfg(test)]
mod events;

#[cfg(test)]
mod auth;

#[cfg(test)]
mod execution;

#[cfg(test)]
mod validators;

#[cfg(test)]
mod profile;

#[cfg(test)]
mod profile_gateway;

#[cfg(test)]
mod provider_catalog_parity;

#[cfg(test)]
mod provider_protocol_filter;

#[cfg(test)]
mod wire_codec;

#[cfg(test)]
mod agui;

#[cfg(test)]
mod ai_tool_model_config;

#[cfg(test)]
mod text_utils;

#[cfg(test)]
mod routing;

#[cfg(test)]
mod marketplace;

#[cfg(test)]
mod gateway_hash;

#[cfg(test)]
mod config_postgres;

#[cfg(test)]
mod permission;

#[cfg(test)]
mod mcp_tool_result_metadata;

#[cfg(test)]
mod secrets;

#[cfg(test)]
mod net;

#[cfg(test)]
mod subprocess;

#[cfg(test)]
mod services_hooks;

#[cfg(test)]
mod services_other;

#[cfg(test)]
mod content_config;

#[cfg(test)]
mod services_plugin;

#[cfg(test)]
mod auth_enums;

#[cfg(test)]
mod ai_content_part;

#[cfg(test)]
mod services_agent;

#[cfg(test)]
mod disk_agent_config;

#[cfg(test)]
mod internal_api_error;

#[cfg(test)]
mod execution_plan;

#[cfg(test)]
mod mcp_capabilities;

#[cfg(test)]
mod auth_permission;

#[cfg(test)]
mod auth_types;

#[cfg(test)]
mod bridge_ids;

#[cfg(test)]
mod bridge_manifest_version;

#[cfg(test)]
mod mcp_deployment;

#[cfg(test)]
mod errors_models;

#[cfg(test)]
mod env_interpolation;

#[cfg(test)]
mod time_format_tests;

#[cfg(test)]
mod users_summary;

#[cfg(test)]
mod oauth_models;

#[cfg(test)]
mod repository_tests;

#[cfg(test)]
mod modules_tests;

#[cfg(test)]
mod ai_sampling;

#[cfg(test)]
mod ai_media_types;

#[cfg(test)]
mod config_environment;

#[cfg(test)]
mod config_rate_limits;

#[cfg(test)]
mod content_models;

#[cfg(test)]
mod services_includable;

#[cfg(test)]
mod paths_tests;

#[cfg(test)]
mod schema_sanitizer;

#[cfg(test)]
mod validators_driven;

#[cfg(test)]
mod wire_streaming;

#[cfg(test)]
mod wire_sse;

#[cfg(test)]
mod profile_validation;

#[cfg(test)]
mod auth_claims;

#[cfg(test)]
mod canonical_request;

#[cfg(test)]
mod ai_template_validation;

#[cfg(test)]
mod events_builders;

#[cfg(test)]
mod mcp_apps;

#[cfg(test)]
mod services_validation;

#[cfg(test)]
mod artifacts_provenance;

#[cfg(test)]
mod events_system;

#[cfg(test)]
mod models_misc_edges;

#[cfg(test)]
mod profile_from_env;

#[cfg(test)]
mod ai_request_response;
