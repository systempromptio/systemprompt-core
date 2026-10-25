//! Core `AuthzDecisionHook` wrapping the in-process [`super::resolver`].
//!
//! `RuleBasedHook` is the canonical RBAC layer: it loads
//! `access_control_rules` for the request's entity, resolves them through the
//! entity's plugin and marketplace parent chain, and emits an
//! `AuthzDecision`. The chain's membership is supplied at construction,
//! because this crate cannot load the services configuration itself, and is
//! fixed for the process lifetime; the loaded chain index is held in a
//! [`ChainIndexCache`] and revalidated against a table fingerprint, and each
//! entity's catalogue row and rules are cached for as long as that index is.
//! Exposed as a hook so extensions can compose it explicitly with their own
//! ABAC predicates via [`super::CompositeAuthzHook`]:
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
use systemprompt_identifiers::{ModelId, ProviderId};
use systemprompt_traits::CacheEpochs;

use super::audit::{AuthzAuditSink, AuthzSource};
use super::error::{AuthzError, AuthzResult};
use super::hook::AuthzDecisionHook;
use super::parent_chain::{
    ChainIndexCache, ChainSources, DecisionFacts, ParentChainIndex, ResolveBase,
};
use super::registry::{AuthzHookContext, AuthzServices};
use super::repository::AccessControlRepository;
use super::rule_based_cache::{EntityRules, EntityRulesCache};
use super::subject::{
    SharedSubjectAttributeProvider, SubjectDimension, dimensions_of, discover_subject_providers,
    gather_subject_attributes,
};
use super::types::{AuthzDecision, AuthzRequest, Decision, DenyReason, EntityKind};

#[derive(Clone)]
pub struct RuleBasedHook {
    repo: AccessControlRepository,
    sink: Arc<dyn AuthzAuditSink>,
    providers: Vec<SharedSubjectAttributeProvider>,
    dimensions: Vec<SubjectDimension>,
    sources: Arc<ChainSources>,
    cache: Arc<ChainIndexCache>,
    entity_cache: Arc<EntityRulesCache>,
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
    pub fn new(
        pool: Arc<PgPool>,
        sink: Arc<dyn AuthzAuditSink>,
        sources: ChainSources,
        epochs: CacheEpochs,
        services: AuthzServices,
    ) -> Self {
        let providers = discover_subject_providers(&AuthzHookContext {
            pool: Arc::clone(&pool),
            sink: Arc::clone(&sink),
            services,
        });
        Self {
            repo: AccessControlRepository::from_pool(pool),
            sink,
            dimensions: dimensions_of(&providers),
            providers,
            sources: Arc::new(sources),
            cache: Arc::new(ChainIndexCache::with_default_bounds(epochs)),
            entity_cache: Arc::new(EntityRulesCache::default()),
        }
    }

    async fn chain_index(&self) -> AuthzResult<Arc<ParentChainIndex>> {
        self.cache.get(&self.repo, Arc::clone(&self.sources)).await
    }

    async fn entity_rules(
        &self,
        index: &Arc<ParentChainIndex>,
        kind: EntityKind,
        id: &str,
    ) -> AuthzResult<Arc<EntityRules>> {
        if let Some(cached) = self.entity_cache.get(index, kind, id) {
            return Ok(cached);
        }
        let entity = self.repo.get_entity(kind, id).await?;
        let rules = self.repo.list_rules_for_entity(kind, id).await?;
        Ok(self
            .entity_cache
            .insert(index, kind, id, EntityRules { entity, rules }))
    }

    async fn fault(&self, req: &AuthzRequest, error: &AuthzError) -> AuthzDecision {
        tracing::warn!(
            entity = %req.entity,
            user_id = %req.user_id,
            error = %error,
            "rule-based authz hook fault",
        );
        let decision = fault_decision(error);
        self.sink
            .record(req, &decision, AuthzSource::RuleBased)
            .await;
        decision
    }

    async fn decide(&self, req: &AuthzRequest) -> AuthzResult<AuthzDecision> {
        let kind = req.entity.kind();
        let id = req.entity.id_str();
        let index = self.chain_index().await?;
        let cached = self.entity_rules(&index, kind, id).await?;
        let EntityRules { entity, rules } = cached.as_ref();
        let attributes = gather_subject_attributes(&self.providers, &req.user_id).await?;
        let model = req.context.gateway_invocation_model();
        let provider = req.context.gateway_invocation_provider();
        let decision = index.resolve(
            kind,
            id,
            ResolveBase {
                rules,
                user_id: &req.user_id,
                user_roles: &req.roles,
                default_included: entity.as_ref().map(|e| e.default_included),
                attributes: &attributes,
                dimensions: &self.dimensions,
                facts: DecisionFacts {
                    model: model.as_ref().map(ModelId::as_str),
                    provider: provider.as_ref().map(ProviderId::as_str),
                },
            },
        );
        Ok(to_authz_decision(req, decision))
    }
}

fn fault_decision(error: &AuthzError) -> AuthzDecision {
    let policy = AuthzSource::RuleBased.policy().to_owned();
    AuthzDecision::Deny {
        reason: DenyReason::HookUnavailable {
            policy: policy.clone(),
            detail: error.to_string(),
        },
        policy,
    }
}

fn to_authz_decision(req: &AuthzRequest, decision: Decision) -> AuthzDecision {
    let policy = AuthzSource::RuleBased.policy().to_owned();
    match decision {
        Decision::Allow { .. } => AuthzDecision::Allow,
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
    }
}

#[async_trait]
impl AuthzDecisionHook for RuleBasedHook {
    async fn evaluate(&self, req: AuthzRequest) -> AuthzDecision {
        match self.decide(&req).await {
            Ok(decision) => {
                self.sink
                    .record(&req, &decision, AuthzSource::RuleBased)
                    .await;
                decision
            },
            Err(error) => self.fault(&req, &error).await,
        }
    }

    async fn probe(&self, req: AuthzRequest) -> AuthzDecision {
        self.decide(&req)
            .await
            .unwrap_or_else(|error| fault_decision(&error))
    }
}
