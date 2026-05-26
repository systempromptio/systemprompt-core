//! Deny-overrides composition of multiple [`AuthzDecisionHook`] impls.
//!
//! [`CompositeAuthzHook`] is an opt-in utility for deployments that combine
//! several independent ABAC overlays (e.g. an ITAR predicate from one
//! extension and a classification predicate from another) without either
//! extension needing to know about the other. The composite iterates its
//! hooks in order, short-circuits on the first `Deny`, and otherwise returns
//! `Allow`. Composition is pure data: callers build the `Vec` themselves at
//! their binary entry point and pass the result to
//! `AppContextBuilder::with_authz_hook`. Core never auto-composes.
//!
//! Each evaluated hook still records its own audit row through whatever
//! `AuthzAuditSink` it was constructed with. Hooks that the short-circuit
//! skipped are not consulted, so no audit row is produced for them — that is
//! intentional: the composite's contract is "the first deny is the final
//! word." If you need every hook to fire regardless, do not use this type.

use async_trait::async_trait;

use super::hook::AuthzDecisionHook;
use super::types::{AuthzDecision, AuthzRequest};
use super::SharedAuthzHook;

#[derive(Debug)]
pub struct CompositeAuthzHook {
    hooks: Vec<SharedAuthzHook>,
}

impl CompositeAuthzHook {
    #[must_use]
    pub fn new(hooks: Vec<SharedAuthzHook>) -> Self {
        Self { hooks }
    }
}

#[async_trait]
impl AuthzDecisionHook for CompositeAuthzHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        for hook in &self.hooks {
            match hook.evaluate(req.clone()).await {
                AuthzDecision::Allow => continue,
                deny @ AuthzDecision::Deny { .. } => return deny,
            }
        }
        AuthzDecision::Allow
    }
}
