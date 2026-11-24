//! Trait impls for [`crate::AppContext`].
//!
//! Split out of `context.rs` so the core type definition stays under
//! the 300-line public-API ceiling.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::HasRouteClassifier;
use systemprompt_models::RouteClassifier;
use systemprompt_traits::{
    AnalyticsProvider, AppContext as AppContextTrait, ConfigProvider, DatabaseHandle,
    FingerprintProvider, UserProvider,
};

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

    fn session_provider(&self) -> Option<Arc<dyn systemprompt_traits::SessionProvider>> {
        let sessions = self
            .kernel()
            .state()
            .find::<systemprompt_traits::DynSessionStore>()?;
        let store: systemprompt_traits::DynSessionStore = Arc::clone(sessions);
        let provider: Arc<dyn systemprompt_traits::SessionProvider> = store;
        Some(provider)
    }

    fn analytics_provider(&self) -> Arc<dyn AnalyticsProvider> {
        Arc::clone(&self.analytics)
    }

    fn fingerprint_provider(&self) -> Option<Arc<dyn FingerprintProvider>> {
        self.kernel()
            .state()
            .find::<Arc<dyn FingerprintProvider>>()
            .cloned()
    }

    fn user_provider(&self) -> Option<Arc<dyn UserProvider>> {
        self.kernel()
            .state()
            .find::<systemprompt_traits::DynUserProvider>()
            .cloned()
    }
}

impl HasRouteClassifier for AppContext {
    type RouteClassifier = Arc<RouteClassifier>;

    fn route_classifier(&self) -> &Self::RouteClassifier {
        &self.route_classifier
    }
}
