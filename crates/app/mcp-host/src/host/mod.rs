//! [`HostContext`]: what a spawned MCP server runs on.
//!
//! A managed MCP server is a least-privilege child of the API. It boots from
//! the environment the API spawned it with and nothing else: the profile path,
//! the database URLs, the deployment's public verification key set
//! (`VERIFICATION_KEYS`, JSON of
//! [`TokenVerificationKeys`](systemprompt_security::keys::TokenVerificationKeys),
//! staged key included) and the secrets its manifest grants
//! ([`GrantedSecrets`]). It never reads the profile's secrets source, and it
//! holds no signing key, token authority, OAuth pepper, manifest seed or
//! licence secret, nor the encryption master key unless that was granted. It
//! constructs no OAuth repository. What only a signer may do (signing a
//! document download or upload link) it asks the API for through its
//! [`LinkIssuer`], presenting the hop
//! token of the call it is serving.
//!
//! The boot sequence is [`HostKernel`](systemprompt_runtime::HostKernel), the
//! runtime's composition stages run over the child's granted environment; the
//! host context adds the
//! [`ExtensionContext`](systemprompt_extension::ExtensionContext) those stages
//! finish over.
//!
//! It runs the API's extension seam discovery over its own
//! [`ExtensionContext`](systemprompt_extension::ExtensionContext), whose
//! token authority only verifies, so documents granted to an extension
//! audience resolve as they do on the API. An escalated notification is left
//! for the API's notifications consumer to deliver
//! (`EscalationDelivery::Consumer`), because the host delivers through no
//! extension channel.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod boot;
mod extension_context;

use std::sync::Arc;

use systemprompt_config::GrantedSecrets;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_files::FilesConfig;
use systemprompt_governance::authz::{SharedAuthzHook, SubjectProviderSet};
use systemprompt_identifiers::{McpServerId, UserId};
use systemprompt_loader::ServicesState;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_manifest::{Config, Profile};
use systemprompt_marketplace::managed::PublicationPolicy;
use systemprompt_mcp::ArtifactIngest;
use systemprompt_mcp::repository::McpSessionRepository;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::documents::DocumentsRuntime;
use systemprompt_runtime::documents::links::{ApiLinkIssuer, LinkIssueError, LinkIssuer};
use systemprompt_runtime::{HostNotifications, HostParts};
use systemprompt_security::credential::TokenCache;
use systemprompt_security::keys::VerificationKeyring;
use systemprompt_traits::{BackgroundTasks, DynToolCallIntentClaims, SecretStore};
use systemprompt_users::UserRepository;

pub use systemprompt_runtime::{HostBootError, PROFILE_ENV};

/// The least-privilege context a spawned MCP server or agent is built on.
#[derive(Clone)]
pub struct HostContext {
    parts: HostParts,
    notifications: HostNotifications,
    extension_state: Dependencies,
    provider_tokens: Arc<TokenCache>,
}

impl std::fmt::Debug for HostContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostContext")
            .field("parts", &self.parts)
            .finish_non_exhaustive()
    }
}

impl HostContext {
    #[must_use]
    pub const fn parts(&self) -> &HostParts {
        &self.parts
    }

    #[must_use]
    pub fn profile(&self) -> &Profile {
        self.parts.profile.profile()
    }

    #[must_use]
    pub const fn services_root(&self) -> &ActiveServicesRoot {
        &self.parts.services_root
    }

    #[must_use]
    pub fn config(&self) -> &Config {
        &self.parts.config
    }

    #[must_use]
    pub const fn system_owner(&self) -> &UserId {
        &self.parts.system_owner
    }

    #[must_use]
    pub const fn app_paths(&self) -> &Arc<AppPaths> {
        &self.parts.app_paths
    }

    #[must_use]
    pub const fn files_config(&self) -> &Arc<FilesConfig> {
        &self.parts.files_config
    }

    #[must_use]
    pub const fn db_pool(&self) -> &DbPool {
        &self.parts.database
    }

    #[must_use]
    pub const fn services_state(&self) -> &ServicesState {
        &self.parts.services
    }

    #[must_use]
    pub fn secrets(&self) -> Arc<dyn SecretStore> {
        let granted: Arc<dyn SecretStore> = Arc::<GrantedSecrets>::clone(&self.parts.secrets);
        granted
    }

    #[must_use]
    pub const fn granted_secrets(&self) -> &Arc<GrantedSecrets> {
        &self.parts.secrets
    }

    #[must_use]
    pub const fn verification_keys(&self) -> &VerificationKeyring {
        &self.parts.verification
    }

    #[must_use]
    pub const fn authz_hook(&self) -> &SharedAuthzHook {
        &self.parts.authz_hook
    }

    #[must_use]
    pub const fn publication_policy(&self) -> &PublicationPolicy {
        &self.parts.publication
    }

    #[must_use]
    pub const fn subject_providers(&self) -> &SubjectProviderSet {
        &self.parts.subject_providers
    }

    #[must_use]
    pub fn artifact_ingest(&self) -> Arc<ArtifactIngest> {
        Arc::clone(&self.parts.artifact_ingest)
    }

    #[must_use]
    pub fn tool_call_intents(&self) -> DynToolCallIntentClaims {
        Arc::clone(&self.parts.tool_call_intents)
    }

    #[must_use]
    pub const fn mcp_session_repository(&self) -> &Arc<McpSessionRepository> {
        &self.parts.mcp_sessions
    }

    #[must_use]
    pub const fn user_repository(&self) -> &Arc<UserRepository> {
        &self.parts.user_repository
    }

    #[must_use]
    pub const fn documents(&self) -> &Arc<DocumentsRuntime> {
        &self.parts.documents
    }

    #[must_use]
    pub const fn notifications(&self) -> &HostNotifications {
        &self.notifications
    }

    #[must_use]
    pub const fn log_sink(&self) -> &systemprompt_logging::LogSink {
        &self.parts.log_sink
    }

    #[must_use]
    pub const fn background_tasks(&self) -> &BackgroundTasks {
        &self.parts.background_tasks
    }

    pub fn link_issuer(&self, server: &McpServerId) -> Result<Arc<dyn LinkIssuer>, LinkIssueError> {
        let issuer: Arc<dyn LinkIssuer> = Arc::new(ApiLinkIssuer::new(
            &self.parts.config.api_internal_url,
            server.clone(),
        )?);
        Ok(issuer)
    }
}
