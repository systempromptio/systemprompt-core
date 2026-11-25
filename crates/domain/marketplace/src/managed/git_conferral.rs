//! [`GitSourceSyncs`]: the issuer of the conferred Git source sync, which
//! this member provides sealed under the `git-source-sync` row of
//! `systemprompt_extension::reserved::INSTALL_CONFERRALS`.
//!
//! A member's handle acts for the system admin alone (the owner installs
//! attribute managed inventory to), never an owner the member names, and
//! writes a `governance_decisions` row (policy `conferred_git_sync`, actor
//! `member:<id>`) before each sync; a failed write fails the sync. It syncs
//! a registered Git source through the
//! [`GitSourceOrchestrator`], which resolves the source's stored credential
//! from the sealed secret store by the source's own host and label; the
//! credential goes to `git` and never to the caller, and a credential that
//! cannot be resolved is reported as unavailable without its secret name.
//! No handle's `Debug` prints the orchestrator or the secret store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_extension::prelude::{ProvideError, Resolved};
use systemprompt_governance::authz::{
    DecisionTag, GovernanceDecisionRecord, GovernanceDecisionRepository,
};
use systemprompt_identifiers::{Actor, ContextId, ExtensionId, UserId};
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_traits::{
    ConferralIssuer, DynConferralIssuer, GitSourceSync, GitSourceSyncError, GitSourceSyncRequest,
    GitSourceSynced, MemberGrant, SecretStore,
};

use super::git_sources::{GitSourceError, GitSourceOrchestrator};
use super::{GitSyncRequest, GitSyncResult, ManagedRepository};

/// The sealed issuer of every member's Git source sync handle.
#[derive(Clone)]
pub struct GitSourceSyncs {
    orchestrator: GitSourceOrchestrator,
    owner: UserId,
    audit: GovernanceDecisionRepository,
}

impl std::fmt::Debug for GitSourceSyncs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GitSourceSyncs(<redacted>)")
    }
}

impl GitSourceSyncs {
    #[must_use]
    pub const fn new(
        orchestrator: GitSourceOrchestrator,
        owner: UserId,
        audit: GovernanceDecisionRepository,
    ) -> Self {
        Self {
            orchestrator,
            owner,
            audit,
        }
    }

    pub(crate) fn provide(
        r: &Resolved<'_>,
    ) -> Result<DynConferralIssuer<dyn GitSourceSync>, ProvideError> {
        let repository = r.get::<Arc<ManagedRepository>>()?;
        let secrets = r.get::<Arc<dyn SecretStore>>()?;
        let paths = r.get::<Arc<AppPaths>>()?;
        let owner = r.get::<SystemAdmin>()?.id().clone();
        let db = r.get::<DbPool>()?;
        Ok(Arc::new(Self::new(
            GitSourceOrchestrator::new(
                repository.as_ref().clone(),
                paths.storage().scratch().to_path_buf(),
                secrets,
            ),
            owner,
            GovernanceDecisionRepository::from_pool(db.write_pool()),
        )))
    }
}

impl ConferralIssuer<dyn GitSourceSync> for GitSourceSyncs {
    fn issue(&self, grant: &MemberGrant) -> Arc<dyn GitSourceSync> {
        Arc::new(MemberGitSourceSync {
            syncs: self.clone(),
            member: grant.member.clone(),
        })
    }
}

struct MemberGitSourceSync {
    syncs: GitSourceSyncs,
    member: ExtensionId,
}

impl std::fmt::Debug for MemberGitSourceSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemberGitSourceSync")
            .field("member", &self.member)
            .finish_non_exhaustive()
    }
}

impl MemberGitSourceSync {
    async fn audit(&self, request: &GitSourceSyncRequest) -> Result<(), GitSourceSyncError> {
        let actor = Actor::job(self.syncs.owner.clone(), format!("member:{}", self.member));
        let context = ContextId::generate();
        let id = format!("git-sync-{}", context.as_str());
        let reason = format!("{} synced Git source {}", self.member, request.source_id);
        let rules = serde_json::json!({
            "member": self.member.as_str(),
            "source_id": request.source_id.as_str(),
            "resource_id": request.resource_id.as_str(),
        });
        self.syncs
            .audit
            .insert(&GovernanceDecisionRecord {
                id: &id,
                actor: &actor,
                session_id: None,
                tool_name: SYNC_POLICY,
                agent_id: None,
                agent_scope: None,
                decision: DecisionTag::Allow,
                policy: SYNC_POLICY,
                reason: &reason,
                evaluated_rules: &rules,
                plugin_id: None,
                act_chain: &[],
                context_id: &context,
                task_id: None,
                trace_id: None,
                client_id: None,
                tool_use_id: None,
            })
            .await
            .map_err(|error| GitSourceSyncError::Audit(Box::new(error)))
    }
}

const SYNC_POLICY: &str = "conferred_git_sync";

fn sync_error(error: GitSourceError) -> GitSourceSyncError {
    match error {
        GitSourceError::NotGitSource => GitSourceSyncError::NotGitSource,
        GitSourceError::CredentialUnresolved | GitSourceError::CredentialNotMigrated { .. } => {
            GitSourceSyncError::CredentialUnavailable
        },
        GitSourceError::Managed(source) => GitSourceSyncError::Failed(Box::new(source)),
    }
}

#[async_trait]
impl GitSourceSync for MemberGitSourceSync {
    async fn sync(
        &self,
        request: &GitSourceSyncRequest,
    ) -> Result<GitSourceSynced, GitSourceSyncError> {
        self.audit(request).await?;
        let outcome = self
            .syncs
            .orchestrator
            .synchronize(
                &self.syncs.owner,
                &GitSyncRequest {
                    source_id: request.source_id.clone(),
                    resource_id: request.resource_id.clone(),
                    upstream_root: request.upstream_root.clone(),
                    upstream_base_revision_id: request.upstream_base_revision_id.clone(),
                },
            )
            .await
            .map_err(sync_error);
        tracing::info!(
            member = %self.member,
            source_id = %request.source_id,
            synced = outcome.is_ok(),
            "conferred Git source sync"
        );
        Ok(match outcome? {
            GitSyncResult::Incoming {
                snapshot_id,
                revision_id,
                commit,
                ..
            } => GitSourceSynced::Incoming {
                snapshot_id,
                revision_id,
                commit,
            },
            GitSyncResult::WithdrawalProposed {
                snapshot_id,
                proposal_id,
                commit,
            } => GitSourceSynced::WithdrawalProposed {
                snapshot_id,
                proposal_id,
                commit,
            },
        })
    }
}
