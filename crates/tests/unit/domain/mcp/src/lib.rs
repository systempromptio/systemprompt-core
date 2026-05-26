//! Unit tests for systemprompt-core-mcp crate
//!
//! Tests cover:
//! - Models: ExecutionStatus, ValidationResultType, MCPService, ToolExecution,
//!   ToolStats
//! - Client types: McpConnectionResult, McpProtocolInfo, ValidationResult
//! - Monitoring: HealthStatus, HealthCheckResult, HealthCheckDetails,
//!   ServiceStatus
//! - Orchestrator: McpEvent, EventBus
//! - Middleware: AuthenticatedRequestContext, AuthResult
//! - Error: McpError, McpResult
//! - Orchestration: McpServerConnectionInfo, ServerStatus, SkillLoadingResult,
//!   McpServiceState

#![allow(clippy::all)]

#[cfg(test)]
mod capabilities;
#[cfg(test)]
mod error;
#[cfg(test)]
mod error_classify;
#[cfg(test)]
mod error_from;
#[cfg(test)]
mod extension;
#[cfg(test)]
mod lib_smoke;
#[cfg(test)]
mod middleware;
#[cfg(test)]
mod models;
#[cfg(test)]
mod orchestration;
#[cfg(test)]
mod resources;
#[cfg(test)]
mod response;
#[cfg(test)]
mod schema;
#[cfg(test)]
mod services;
