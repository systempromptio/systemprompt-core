//! Unit tests for systemprompt-core-mcp crate
//!
//! Tests cover:
//! - Models: ExecutionStatus, ValidationResultType, MCPService, ToolExecution, ToolStats
//! - Client types: McpConnectionResult, McpProtocolInfo, ValidationResult
//! - Monitoring: HealthStatus, HealthCheckResult, HealthCheckDetails, ServiceStatus
//! - Orchestrator: McpEvent, EventBus
//! - Middleware: AuthenticatedRequestContext, AuthResult
//! - Error: McpError, McpResult
//! - Orchestration: McpServerConnectionInfo, ServerStatus, SkillLoadingResult, McpServiceState

#![allow(clippy::all)]

mod error;
mod middleware;
mod models;
mod orchestration;
mod services;
