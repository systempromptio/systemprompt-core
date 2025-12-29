//! Unit tests for systemprompt-core-mcp crate
//!
//! Tests cover:
//! - Models: ExecutionStatus, ValidationResultType, MCPService, ToolExecution, ToolStats
//! - Client types: McpConnectionResult, McpProtocolInfo, ValidationResult
//! - Monitoring: HealthStatus, HealthCheckResult, HealthCheckDetails, ServiceStatus
//! - Orchestrator: McpEvent, EventBus
//! - Middleware: AuthenticatedRequestContext, AuthResult

#![allow(clippy::all)]

mod middleware;
mod models;
mod services;
