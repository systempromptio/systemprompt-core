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
//! # Audit semantics
//!
//! Each *evaluated* hook records its own audit row through whatever
//! `AuthzAuditSink` it was constructed with, keyed by `trace_id`. An audit
//! reader reconstructs the composite's behaviour for a single request by
//! grouping rows on `trace_id`: an N-hook composite produces between one and
//! N rows, depending on where (or whether) a deny short-circuited the chain.
//!
//! Hooks that the short-circuit skipped are not consulted and produce no
//! row — intentional: the composite's contract is "the first deny is the
//! final word", and a skipped hook has nothing to say. The composite itself
//! does NOT record a "composite fired" row — there is no composer-level
//! audit. A deployment that wants one wraps the composite in its own
//! logging hook.
//!
//! If every hook must fire regardless of an earlier deny, do not use this
//! type — write a custom composer.

use async_trait::async_trait;

use super::hook::{AuthzDecisionHook, SharedAuthzHook};
use super::types::{AuthzDecision, AuthzRequest};

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
            if let deny @ AuthzDecision::Deny { .. } = hook.evaluate(req.clone()).await {
                return deny;
            }
        }
        AuthzDecision::Allow
    }
}
