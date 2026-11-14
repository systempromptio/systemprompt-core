//! [`ExtensionContext`] for a [`HostContext`], so a managed MCP server runs
//! the same extension seam discovery as the API.
//!
//! The host hands extension factories what it holds and nothing the API
//! alone holds. Its token authority verifies against the public key set and
//! refuses to sign ([`VerifyingTokenAuthority`]). Its kernel state carries
//! no OAuth or AI repositories, so a factory that looks them up finds none.
//! An extension that needs either fails in the host instead of running with a
//! narrower view. The host runs every extension's `provide` over this context
//! before its seams are discovered, so extension state resolves as it does on
//! the API.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::{Extension, ExtensionContext};
use systemprompt_identifiers::{InstanceId, UserId};
use systemprompt_manifest::services::ServicesView;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_manifest::{Config, Profile};
use systemprompt_models::auth::TokenAuthority;
use systemprompt_models::authz::SubjectAttributeSource;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_security::jwt::VerifyingTokenAuthority;
use systemprompt_traits::{
    BackgroundTasks, ConfigProvider, DatabaseHandle, FileStorage, ProviderTokenCache, SecretStore,
};

use super::HostContext;

impl ExtensionContext for HostContext {
    fn system_owner_id(&self) -> UserId {
        self.parts.system_owner.clone()
    }

    fn config(&self) -> Arc<dyn ConfigProvider> {
        let provider: Arc<dyn ConfigProvider> = Arc::<Config>::clone(&self.parts.config);
        provider
    }

    fn profile(&self) -> Arc<Profile> {
        Arc::new(self.parts.profile.profile().clone())
    }

    fn app_config(&self) -> Arc<Config> {
        Arc::clone(&self.parts.config)
    }

    fn services_root(&self) -> ActiveServicesRoot {
        self.parts.services_root.clone()
    }

    fn token_authority(&self) -> Arc<dyn TokenAuthority> {
        let tokens: Arc<dyn TokenAuthority> =
            Arc::<VerifyingTokenAuthority>::clone(&self.parts.tokens);
        tokens
    }

    fn database(&self) -> Arc<dyn DatabaseHandle> {
        let concrete = Arc::clone(&self.parts.database);
        let handle: Arc<dyn DatabaseHandle> = concrete;
        handle
    }

    fn file_storage(&self) -> Arc<dyn FileStorage> {
        Arc::clone(&self.parts.file_storage)
    }

    fn instance_id(&self) -> InstanceId {
        self.parts.config.instance_id.clone()
    }

    fn background_tasks(&self) -> BackgroundTasks {
        self.parts.background_tasks.clone()
    }

    fn secrets(&self) -> Arc<dyn SecretStore> {
        Self::secrets(self)
    }

    fn get_extension(&self, id: &str) -> Option<Arc<dyn Extension>> {
        self.parts.extensions.get(id).cloned()
    }

    fn services(&self) -> ServicesView {
        self.parts.services.view()
    }

    fn subject_attributes(&self) -> Arc<dyn SubjectAttributeSource> {
        Arc::new(self.parts.subjects.clone())
    }

    fn extension_state(&self) -> Dependencies {
        self.extension_state.clone()
    }

    fn provider_tokens(&self) -> Arc<dyn ProviderTokenCache> {
        let concrete = Arc::clone(&self.provider_tokens);
        let cache: Arc<dyn ProviderTokenCache> = concrete;
        cache
    }
}
