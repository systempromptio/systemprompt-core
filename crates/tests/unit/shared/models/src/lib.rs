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
mod ai;

#[cfg(test)]
mod api;

#[cfg(test)]
mod artifacts;

#[cfg(test)]
mod artifacts_extended;

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
mod agui;

#[cfg(test)]
mod ai_tool_model_config;

#[cfg(test)]
mod text_utils;

#[cfg(test)]
mod routing;

#[cfg(test)]
mod modules;
