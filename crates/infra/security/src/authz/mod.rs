//! Unified authorization decision plane.
//!
//! Houses the deny-overrides resolver, `access_control_rules` repository,
//! and [`AuthzDecisionHook`] extension surface shared by the gateway
//! `/v1/messages` proxy and the MCP RBAC middleware. Both call
//! [`resolve`] with different `entity_type` / `entity_id` pairs against
//! the same table and audit shape.
//!
//! # Three-layer model
//!
//! Authorization runs in three layers, in order. Each can only tighten an
//! earlier `Allow`; none can loosen a `Deny`.
//!
//! 1. **PBAC** — `Permission` enum on the JWT `scope` claim, enforced at the
//!    route boundary by `with_auth(scope)`. Lives in core. Always on.
//! 2. **RBAC** — `access_control_rules` table evaluated by [`resolve`]
//!    against `AuthzRequest.{user_id, roles, department}`. Lives in core.
//!    Always on after PBAC; empty table = allow-all at this layer.
//! 3. **ABAC hook** — [`AuthzDecisionHook::evaluate`] called after RBAC. Lives
//!    in extensions; core ships [`DenyAllHook`], [`AllowAllHook`],
//!    [`WebhookHook`], and the [`CompositeAuthzHook`] composer for the
//!    multi-extension case.

pub mod audit;
pub mod composite;
pub mod config;
pub mod error;
pub mod extension;
pub mod hook;
pub mod ingestion;
pub mod registry;
pub mod repository;
pub mod resolver;
pub mod runtime;
pub mod types;

pub use audit::{
    AUDIT_WRITE_FAILED_TOTAL, AuthzAuditSink, AuthzSource, DbAuditSink, GovernanceDecisionRecord,
    GovernanceDecisionRepository, NullAuditSink, insert_governance_decision,
};
pub use composite::CompositeAuthzHook;
pub use config::{AccessControlConfig, DepartmentEntry, RuleEntry};
pub use error::{AuthzBootstrapError, AuthzError, AuthzResult};
pub use extension::AuthzExtension;
pub use hook::{AllowAllHook, AuthzDecisionHook, DenyAllHook, SharedAuthzHook, WebhookHook};
pub use ingestion::{AccessControlIngestionService, IngestOptions, IngestReport};
pub use registry::{AuthzHookContext, AuthzHookRegistration, discover_authz_hook};
pub use repository::{AccessControlRepository, UpsertRuleParams};
pub use resolver::{ResolveInput, resolve};
pub use runtime::build_authz_hook;
pub use types::{
    Access, AccessRule, AuthzContext, AuthzDecision, AuthzRequest, Decision, DecisionTag,
    DenyReason, EntityKind, EntityRef, EntityRow, MatchedBy, RuleType,
};
