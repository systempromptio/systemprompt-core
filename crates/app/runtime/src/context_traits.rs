//! Trait impls for [`crate::AppContext`].
//!
//! Split out of `context.rs` so the core type definition stays under
//! the 300-line public-API ceiling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_analytics::{AnalyticsService, FingerprintRepository};
use systemprompt_extension::{
    Extension, ExtensionContext, HasAnalytics, HasFingerprint, HasRouteClassifier, HasUserService,
};
use systemprompt_models::RouteClassifier;
use systemprompt_traits::{
    AnalyticsProvider, AppContext as AppContextTrait, ConfigProvider, DatabaseHandle,
    FingerprintProvider, UserProvider,
};
use systemprompt_users::UserService;

use crate::AppContext;

impl AppContextTrait for AppContext {
    fn config(&self) -> Arc<dyn ConfigProvider> {
        let concrete = Arc::clone(&self.cfg.config);
        let provider: Arc<dyn ConfigProvider> = concrete;
        provider
    }

    fn database_handle(&self) -> Arc<dyn DatabaseHandle> {
        let concrete = Arc::clone(&self.data.database);
        let handle: Arc<dyn DatabaseHandle> = concrete;
        handle
    }

    fn session_provider(&self) -> Option<Arc<dyn systemprompt_traits::SessionProvider>> {
        Some(self.session_store())
    }

    fn analytics_provider(&self) -> Option<Arc<dyn AnalyticsProvider>> {
        let concrete = Arc::clone(&self.data.analytics_service);
        let provider: Arc<dyn AnalyticsProvider> = concrete;
        Some(provider)
    }

    fn fingerprint_provider(&self) -> Option<Arc<dyn FingerprintProvider>> {
        let concrete = Arc::clone(self.data.fingerprint_repo.as_ref()?);
        let provider: Arc<dyn FingerprintProvider> = concrete;
        Some(provider)
    }

    fn user_provider(&self) -> Option<Arc<dyn UserProvider>> {
        let concrete = Arc::clone(self.data.user_service.as_ref()?);
        let provider: Arc<dyn UserProvider> = concrete;
        Some(provider)
    }
}

impl ExtensionContext for AppContext {
    fn services(&self) -> systemprompt_manifest::services::ServicesView {
        self.services_state().view()
    }

    fn subject_attributes(&self) -> Arc<dyn systemprompt_traits::SubjectAttributes> {
        Arc::new(self.subject_providers().clone())
    }

    fn provider_tokens(&self) -> Arc<dyn systemprompt_traits::ProviderTokenCache> {
        Arc::<systemprompt_security::credential::TokenCache>::clone(self.connector_tokens())
    }

    fn system_owner_id(&self) -> systemprompt_identifiers::UserId {
        self.system_admin().id().clone()
    }

    fn config(&self) -> Arc<dyn ConfigProvider> {
        let concrete = Arc::clone(&self.cfg.config);
        let provider: Arc<dyn ConfigProvider> = concrete;
        provider
    }

    fn database(&self) -> Arc<dyn DatabaseHandle> {
        let concrete = Arc::clone(&self.data.database);
        let handle: Arc<dyn DatabaseHandle> = concrete;
        handle
    }

    fn file_storage(&self) -> Arc<dyn systemprompt_traits::FileStorage> {
        Arc::clone(Self::file_storage(self))
    }

    fn instance_id(&self) -> systemprompt_identifiers::InstanceId {
        self.config().instance_id.clone()
    }

    fn background_tasks(&self) -> systemprompt_traits::BackgroundTasks {
        Self::background_tasks(self).clone()
    }

    fn instances(&self) -> systemprompt_traits::DynInstanceRegistry {
        let concrete = Arc::clone(self.service_repository());
        let registry: systemprompt_traits::DynInstanceRegistry = concrete;
        registry
    }

    fn leases(&self) -> systemprompt_traits::DynLeaseRepository {
        let concrete = Arc::clone(&self.coordination_repositories().leases);
        let leases: systemprompt_traits::DynLeaseRepository = concrete;
        leases
    }

    fn advisory_locks(&self) -> systemprompt_traits::DynAdvisoryLocks {
        let concrete = Arc::clone(&self.coordination_repositories().locks);
        let locks: systemprompt_traits::DynAdvisoryLocks = concrete;
        locks
    }

    fn secret(&self, name: &str) -> Option<systemprompt_traits::ExtensionSecret> {
        let secrets = match systemprompt_config::SecretsBootstrap::get() {
            Ok(secrets) => secrets,
            Err(error) => {
                tracing::warn!(%error, secret = name, "extension secret requested before secrets loaded");
                return None;
            },
        };
        secrets
            .custom_secret(name)
            .map(|value| systemprompt_traits::ExtensionSecret::new(value.clone()))
    }

    fn get_extension(&self, id: &str) -> Option<Arc<dyn Extension>> {
        self.plugins.extension_registry.get(id).cloned()
    }

    fn config_planes(
        &self,
    ) -> systemprompt_provider_contracts::ConfigPlaneResult<crate::ConfigPlaneSet> {
        Self::config_planes(self)
    }
}

impl HasAnalytics for AppContext {
    type Analytics = Arc<AnalyticsService>;

    fn analytics(&self) -> &Self::Analytics {
        &self.data.analytics_service
    }
}

impl HasFingerprint for AppContext {
    type Fingerprint = Arc<FingerprintRepository>;

    fn fingerprint(&self) -> Option<&Self::Fingerprint> {
        self.data.fingerprint_repo.as_ref()
    }
}

impl HasUserService for AppContext {
    type UserService = Arc<UserService>;

    fn user_service(&self) -> Option<&Self::UserService> {
        self.data.user_service.as_ref()
    }
}

impl HasRouteClassifier for AppContext {
    type RouteClassifier = Arc<RouteClassifier>;

    fn route_classifier(&self) -> &Self::RouteClassifier {
        &self.cfg.route_classifier
    }
}
