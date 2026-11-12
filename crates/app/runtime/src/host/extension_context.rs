//! [`ExtensionContext`] for a [`HostContext`], so a managed MCP server runs
//! the same extension seam discovery as the API.
//!
//! The host hands extension factories what it holds and nothing the API
//! alone holds. Its token authority verifies against the public key set and
//! refuses to sign ([`VerifyingTokenAuthority`]). Its dependency set carries
//! no OAuth or AI repositories, so a factory that looks them up finds none.
//! An extension that needs either fails in the host instead of running with a
//! narrower view.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::{Extension, ExtensionContext, ExtensionRegistry};
use systemprompt_identifiers::{InstanceId, UserId};
use systemprompt_manifest::services::ServicesView;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_manifest::{Config, Profile};
use systemprompt_models::auth::TokenAuthority;
use systemprompt_models::authz::{SubjectAttributeSource, SubjectSourceHandle};
use systemprompt_provider_contracts::Dependencies;
use systemprompt_security::credential::TokenCache;
use systemprompt_security::jwt::VerifyingTokenAuthority;
use systemprompt_traits::{
    BackgroundTasks, ConfigProvider, DatabaseHandle, FileStorage, ProviderTokenCache, SecretStore,
};

use super::HostContext;

#[derive(Clone)]
pub(super) struct ExtensionHandles {
    pub(super) tokens: Arc<VerifyingTokenAuthority>,
    pub(super) file_storage: Arc<dyn FileStorage>,
    pub(super) extensions: Arc<ExtensionRegistry>,
    pub(super) subjects: SubjectSourceHandle,
    pub(super) provider_tokens: Arc<TokenCache>,
    pub(super) system_owner: UserId,
}

impl ExtensionContext for HostContext {
    fn system_owner_id(&self) -> UserId {
        self.extension.system_owner.clone()
    }

    fn config(&self) -> Arc<dyn ConfigProvider> {
        let provider: Arc<dyn ConfigProvider> = Arc::<Config>::clone(&self.config);
        provider
    }

    fn profile(&self) -> Arc<Profile> {
        Arc::new(self.profile.profile().clone())
    }

    fn app_config(&self) -> Arc<Config> {
        Arc::clone(&self.config)
    }

    fn services_root(&self) -> ActiveServicesRoot {
        self.services_root.clone()
    }

    fn token_authority(&self) -> Arc<dyn TokenAuthority> {
        let tokens: Arc<dyn TokenAuthority> =
            Arc::<VerifyingTokenAuthority>::clone(&self.extension.tokens);
        tokens
    }

    fn database(&self) -> Arc<dyn DatabaseHandle> {
        let concrete = Arc::clone(&self.database);
        let handle: Arc<dyn DatabaseHandle> = concrete;
        handle
    }

    fn file_storage(&self) -> Arc<dyn FileStorage> {
        Arc::clone(&self.extension.file_storage)
    }

    fn instance_id(&self) -> InstanceId {
        self.config.instance_id.clone()
    }

    fn background_tasks(&self) -> BackgroundTasks {
        self.background_tasks.clone()
    }

    fn secrets(&self) -> Arc<dyn SecretStore> {
        Self::secrets(self)
    }

    fn dependencies(&self) -> Dependencies {
        let mut dependencies = Dependencies::new()
            .with(Arc::clone(&self.database))
            .with(Arc::clone(&self.app_paths))
            .with(Arc::clone(&self.files_config))
            .with(Arc::new(self.profile().notifications));
        if self.files_config.documents_config().is_enabled() {
            dependencies.insert(Arc::clone(self.documents.service()));
        }
        dependencies
    }

    fn get_extension(&self, id: &str) -> Option<Arc<dyn Extension>> {
        self.extension.extensions.get(id).cloned()
    }

    fn services(&self) -> ServicesView {
        self.services.view()
    }

    fn subject_attributes(&self) -> Arc<dyn SubjectAttributeSource> {
        Arc::new(self.extension.subjects.clone())
    }

    fn provider_tokens(&self) -> Arc<dyn ProviderTokenCache> {
        let concrete = Arc::clone(&self.extension.provider_tokens);
        let cache: Arc<dyn ProviderTokenCache> = concrete;
        cache
    }
}
