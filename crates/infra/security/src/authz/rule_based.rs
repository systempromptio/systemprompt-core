//! Core `AuthzDecisionHook` wrapping the in-process [`super::resolver`].
//!
//! `RuleBasedHook` is the canonical RBAC layer: it loads
//! `access_control_rules` for the request's entity, resolves them through the
//! entity's plugin and marketplace parent chain, and emits an
//! `AuthzDecision`. The chain's membership is supplied at construction,
//! because this crate cannot load the services configuration itself, and is
//! fixed for the process lifetime; the loaded chain index is held in a
//! [`ChainIndexCache`] and revalidated against a table fingerprint. Exposed as
//! a hook so extensions can compose it explicitly with their own ABAC
//! predicates via [`super::CompositeAuthzHook`]:
//!
//! ```ignore
//! let composite = CompositeAuthzHook::new(vec![
//!     Arc::new(RuleBasedHook::new(pool.clone(), sink.clone())),
//!     Arc::new(MyAbacHook::new(...)),
//! ]);
//! ```
//!
//! Put `RuleBasedHook` first so a coarse-grained RBAC reject short-circuits
//! the chain before any per-attribute lookup runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;

use super::audit::{AuthzAuditSink, AuthzSource};
use super::error::{AuthzError, AuthzResult};
use super::hook::AuthzDecisionHook;
use super::parent_chain::{ChainIndexCache, ChainSources, ParentChainIndex, ResolveBase};
use super::registry::AuthzHookContext;
use super::repository::AccessControlRepository;
use super::subject::{
    SharedSubjectAttributeProvider, SubjectDimension, dimensions_of, discover_subject_providers,
    gather_subject_attributes,
};
use super::types::{AuthzDecision, AuthzRequest, Decision, DenyReason};

#[derive(Clone)]
pub struct RuleBasedHook {
    repo: AccessControlRepository,
    sink: Arc<dyn AuthzAuditSink>,
    providers: Vec<SharedSubjectAttributeProvider>,
    dimensions: Vec<SubjectDimension>,
    sources: Arc<ChainSources>,
    cache: Arc<ChainIndexCache>,
}

impl std::fmt::Debug for RuleBasedHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuleBasedHook")
            .field("repo", &self.repo)
            .field("sink", &self.sink)
            .field("providers", &self.providers)
            .field("dimensions", &self.dimensions)
            .finish_non_exhaustive()
    }
}

impl RuleBasedHook {
    #[must_use]
    pub fn new(pool: Arc<PgPool>, sink: Arc<dyn AuthzAuditSink>, sources: ChainSources) -> Self {
        let providers = discover_subject_providers(&AuthzHookContext {
            pool: Arc::clone(&pool),
            sink: Arc::clone(&sink),
        });
        Self {
            repo: AccessControlRepository::from_pool(pool),
            sink,
            dimensions: dimensions_of(&providers),
            providers,
            sources: Arc::new(sources),
            cache: Arc::new(ChainIndexCache::default()),
        }
    }

    async fn chain_index(&self) -> AuthzResult<Arc<ParentChainIndex>> {
        self.cache.get(&self.repo, Arc::clone(&self.sources)).await
    }

    // Why: takes the typed error rather than a rendered string so the reason
    // reaching the audit row names the cause. Stringifying at the call site put
    // it in a log line and left every fault row identical.
    async fn fault(&self, req: &AuthzRequest, error: &AuthzError) -> AuthzDecision {
        let policy = AuthzSource::RuleBased.policy().to_owned();
        let detail = error.to_string();
        let decision = AuthzDecision::Deny {
            reason: DenyReason::HookUnavailable {
                policy: policy.clone(),
                detail: detail.clone(),
            },
            policy,
        };
        tracing::warn!(
            entity = %req.entity,
            user_id = %req.user_id,
            error = %detail,
            "rule-based authz hook fault",
        );
        self.sink
            .record(req, &decision, AuthzSource::RuleBased)
            .await;
        decision
    }
}

#[async_trait]
impl AuthzDecisionHook for RuleBasedHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        let kind = req.entity.kind();
        let id = req.entity.id_str();

        let entity = match self.repo.get_entity(kind, id).await {
            Ok(row) => row,
            Err(err) => return self.fault(&req, &err).await,
        };
        let rules = match self.repo.list_rules_for_entity(kind, id).await {
            Ok(rules) => rules,
            Err(err) => return self.fault(&req, &err).await,
        };

        let index = match self.chain_index().await {
            Ok(index) => index,
            Err(err) => return self.fault(&req, &err).await,
        };

        let attributes = gather_subject_attributes(&self.providers, &req.user_id).await;
        let decision = index.resolve(
            kind,
            id,
            ResolveBase {
                rules: &rules,
                user_id: &req.user_id,
                user_roles: &req.roles,
                default_included: entity.map(|e| e.default_included),
                attributes: &attributes,
                dimensions: &self.dimensions,
            },
        );

        let policy = AuthzSource::RuleBased.policy().to_owned();
        let authz_decision = match decision {
            Decision::Allow { .. } => AuthzDecision::Allow,
            // Why: warn is an allow by construction. This plane has no warn
            // verdict of its own, so the reason is logged here or it is lost —
            // the rule resolver does not write the governance audit row.
            Decision::Warn { reason } => {
                tracing::warn!(
                    entity = %req.entity,
                    user_id = %req.user_id,
                    %reason,
                    "access rule evaluated to warn; allowing the request"
                );
                AuthzDecision::Allow
            },
            Decision::Deny { reason } => AuthzDecision::Deny { reason, policy },
            // Why: the rule resolver answers "may this subject reach this
            // entity", which has no third answer — only the governance chain's
            // `require_approval` returns `Pending`, and it never runs here. A
            // hold reaching this plane means a policy was mounted where it
            // cannot be honoured, so it degrades to a deny rather than an
            // allow.
            Decision::Pending { reason } => {
                tracing::error!(
                    %reason,
                    "a governance hold reached the rule-based resolver, which cannot park a \
                     request; refusing it"
                );
                AuthzDecision::Deny {
                    reason: DenyReason::PolicyViolation {
                        policy: "require_approval".to_owned(),
                        detail: std::borrow::Cow::Borrowed(
                            "approval required, but this enforcement point cannot hold a request",
                        ),
                    },
                    policy,
                }
            },
        };
        self.sink
            .record(&req, &authz_decision, AuthzSource::RuleBased)
            .await;
        authz_decision
    }
}
