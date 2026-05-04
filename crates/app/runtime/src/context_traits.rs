//! Trait impls for [`crate::AppContext`].
//!
//! Split out of `context.rs` so the core type definition stays under
//! the 300-line public-API ceiling.

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
        let concrete = Arc::clone(&self.config);
        let provider: Arc<dyn ConfigProvider> = concrete;
        provider
    }

    fn database_handle(&self) -> Arc<dyn DatabaseHandle> {
        let concrete = Arc::clone(&self.database);
        let handle: Arc<dyn DatabaseHandle> = concrete;
        handle
    }

    fn analytics_provider(&self) -> Option<Arc<dyn AnalyticsProvider>> {
        let concrete = Arc::clone(&self.analytics_service);
        let provider: Arc<dyn AnalyticsProvider> = concrete;
        Some(provider)
    }

    fn fingerprint_provider(&self) -> Option<Arc<dyn FingerprintProvider>> {
        let concrete = Arc::clone(self.fingerprint_repo.as_ref()?);
        let provider: Arc<dyn FingerprintProvider> = concrete;
        Some(provider)
    }

    fn user_provider(&self) -> Option<Arc<dyn UserProvider>> {
        let concrete = Arc::clone(self.user_service.as_ref()?);
        let provider: Arc<dyn UserProvider> = concrete;
        Some(provider)
    }
}

impl ExtensionContext for AppContext {
    fn config(&self) -> Arc<dyn ConfigProvider> {
        let concrete = Arc::clone(&self.config);
        let provider: Arc<dyn ConfigProvider> = concrete;
        provider
    }

    fn database(&self) -> Arc<dyn DatabaseHandle> {
        let concrete = Arc::clone(&self.database);
        let handle: Arc<dyn DatabaseHandle> = concrete;
        handle
    }

    fn get_extension(&self, id: &str) -> Option<Arc<dyn Extension>> {
        self.extension_registry.get(id).cloned()
    }
}

impl HasAnalytics for AppContext {
    type Analytics = Arc<AnalyticsService>;

    fn analytics(&self) -> &Self::Analytics {
        &self.analytics_service
    }
}

impl HasFingerprint for AppContext {
    type Fingerprint = Arc<FingerprintRepository>;

    fn fingerprint(&self) -> Option<&Self::Fingerprint> {
        self.fingerprint_repo.as_ref()
    }
}

impl HasUserService for AppContext {
    type UserService = Arc<UserService>;

    fn user_service(&self) -> Option<&Self::UserService> {
        self.user_service.as_ref()
    }
}

impl HasRouteClassifier for AppContext {
    type RouteClassifier = Arc<RouteClassifier>;

    fn route_classifier(&self) -> &Self::RouteClassifier {
        &self.route_classifier
    }
}
