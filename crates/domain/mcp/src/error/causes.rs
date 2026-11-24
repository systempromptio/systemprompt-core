//! The causes an [`McpDomainError`](super::McpDomainError) variant carries:
//! schema and registry violations, configuration issues, auth requirements
//! and per-service start failures.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_identifiers::AgentName;

use super::McpDomainError;

#[derive(Debug, thiserror::Error)]
pub enum SchemaViolation {
    #[error("Schema file not found: {file} (full path: {})", path.display())]
    FileNotFound { file: String, path: PathBuf },
    #[error("Schema file is empty: {file}")]
    FileEmpty { file: String },
    #[error("Schema must start with CREATE TABLE statement")]
    NotLeadingCreateTable,
    #[error("Schema must contain at least one CREATE TABLE statement")]
    NoCreateTableStatement,
    #[error("No CREATE TABLE statements found in schema")]
    NoTables,
    #[error(
        "Table name '{table}' must start with module prefix '{prefix}' (from module '{module}')"
    )]
    TablePrefix {
        table: String,
        prefix: String,
        module: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryViolation {
    #[error("Port conflicts detected: {}", conflicts.join(", "))]
    PortConflicts { conflicts: Vec<String> },
    #[error("Invalid server configurations:\n{}", issues.join("\n"))]
    ServerConfigs { issues: Vec<String> },
    #[error("OAuth configuration issues:\n{}", issues.join("\n"))]
    OAuthConfigs { issues: Vec<String> },
    #[error("Server type validation issues:\n{}", issues.join("\n"))]
    ServerTypes { issues: Vec<String> },
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationIssue {
    #[error(
        "{server}: server declares no {field}; external MCP servers are proxied to their \
         endpoint and are never spawned locally"
    )]
    NotSpawnable { server: String, field: &'static str },
    #[error("profile path is not valid UTF-8 - cannot spawn MCP server")]
    ProfilePathNotUtf8,
    #[error("MCP server '{server}' has no external_auth accessor to mint a per-user bearer")]
    NoExternalAuthAccessor { server: String },
    #[error(
        "secret `{secret}` is not configured; external MCP server '{server}' cannot resolve its \
         bearer"
    )]
    BrokerSecretMissing {
        secret: &'static str,
        server: String,
    },
    #[error("{server}: internal MCP server declares neither a binary nor toolsets")]
    NoBinaryOrToolsets { server: String },
    #[error("Agent {agent} not found in services.yaml")]
    AgentNotFound { agent: AgentName },
    #[error("No deployment configuration found for server: {server}")]
    MissingDeployment { server: String },
}

#[derive(Debug, thiserror::Error)]
pub enum AuthRequirement {
    #[error("external MCP server '{server}' requires an authenticated user to resolve its bearer")]
    ExternalBearer { server: String },
    #[error("User JWT required for authenticated MCP calls")]
    UserJwt,
}

#[derive(Debug, thiserror::Error)]
#[error("{service} ({source})")]
pub struct ServiceStartFailure {
    pub service: String,
    #[source]
    pub source: Box<McpDomainError>,
}

impl ServiceStartFailure {
    pub fn new(service: impl Into<String>, source: McpDomainError) -> Self {
        Self {
            service: service.into(),
            source: Box::new(source),
        }
    }
}

#[derive(Debug)]
pub struct ServiceStartFailures(pub Vec<ServiceStartFailure>);

impl std::fmt::Display for ServiceStartFailures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} MCP service(s): ", self.0.len())?;
        for (index, failure) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{failure}")?;
        }
        Ok(())
    }
}
