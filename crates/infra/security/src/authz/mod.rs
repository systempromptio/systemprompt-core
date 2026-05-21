//! Unified authorization decision plane.
//!
//! Houses the deny-overrides resolver, `access_control_rules` repository,
//! and [`AuthzDecisionHook`] extension surface shared by the gateway
//! `/v1/messages` proxy and the MCP RBAC middleware. Both call
//! [`resolve`] with different `entity_type` / `entity_id` pairs against
//! the same table and audit shape.

pub mod audit;
pub mod config;
pub mod error;
pub mod extension;
pub mod hook;
pub mod ingestion;
pub mod repository;
pub mod resolver;
pub mod runtime;
pub mod types;

pub use audit::{
    AuthzAuditSink, AuthzSource, DbAuditSink, GovernanceDecisionRecord,
    GovernanceDecisionRepository, NullAuditSink, insert_governance_decision,
};
pub use config::{AccessControlConfig, DepartmentEntry, RuleEntry};
pub use error::{AuthzBootstrapError, AuthzError, AuthzResult};
pub use extension::AuthzExtension;
pub use hook::{AllowAllHook, AuthzDecisionHook, DenyAllHook, WebhookHook};
pub use ingestion::{AccessControlIngestionService, IngestOptions, IngestReport};
pub use repository::{AccessControlRepository, UpsertRuleParams};
pub use resolver::resolve;
pub use runtime::{
    AuthzHookInstalled, clear_global_hook, global_hook, install_from_governance_config,
    install_global_hook,
};
pub use types::{Access, AccessRule, AuthzDecision, AuthzRequest, Decision, EntityKind, RuleType};
