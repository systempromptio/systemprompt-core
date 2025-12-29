pub mod agents;
/// Integration tests root module
///
/// Organized by domain:
/// - ai: AI request lifecycle, tool calls, usage analytics
/// - analytics: Session creation, events, endpoints, AI usage, UTM, GeoIP,
///   integrity
/// - agents: A2A protocol, conversation, tasks, messages, tools, streaming
/// - auth: OAuth flow, JWT validation, session management, permissions
/// - content: Blog, static pages, ingestion, rendering
/// - files: File repository and service tests
/// - mcp: MCP server lifecycle, tools, resources, prompts
/// - database: Foreign keys, constraints, orphaned records
/// - models: Shared model unit tests (RequestContext, Permission, Config, etc.)
/// - traits: Shared trait unit tests (ArtifactSupport, etc.)
/// - users: User management, ban handling, admin operations
pub mod ai;
pub mod analytics;
pub mod auth;
pub mod common;
pub mod content;
pub mod database;
pub mod files;
pub mod mcp;
pub mod models;
pub mod traits;
pub mod users;
