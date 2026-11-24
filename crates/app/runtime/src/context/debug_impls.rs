//! Hand-written `Debug` implementations for the application context planes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{AppContext, ConfigPlane, DataPlane, Plugins, Subsystems};

impl std::fmt::Debug for AppContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContext")
            .field("config", &"Config")
            .field("database", &"DbPool")
            .field("kernel", &self.kernel)
            .field("admitted_members", &self.admitted.built().members().count())
            .field("route_classifier", &"RouteClassifier")
            .field("app_paths", &"AppPaths")
            .field("event_bridge", &self.subsystems.event_bridge.status())
            .field("system_admin", &self.subsystems.system_admin.username())
            .field("authz_hook", &"SharedAuthzHook")
            .field("signing_keys", &self.subsystems.signing_keys)
            .field("governance", &self.subsystems.governance)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for DataPlane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataPlane")
            .field("database", &"DbPool")
            .field("service_repository", &"ServiceRepository")
            .field("coordination", &"CoordinationRepositories")
            .finish()
    }
}

impl std::fmt::Debug for ConfigPlane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigPlane")
            .field("boot", &self.boot)
            .field("config", &"Config")
            .field("app_paths", &"AppPaths")
            .field("route_classifier", &"RouteClassifier")
            .field("build", &self.build)
            .finish()
    }
}

impl std::fmt::Debug for Plugins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plugins")
            .field("subject_providers", &self.subject_providers)
            .field("subjects", &self.subjects)
            .finish()
    }
}

impl std::fmt::Debug for Subsystems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Subsystems");
        debug
            .field("system_admin", &self.system_admin.username())
            .field("logging", &self.logging)
            .field("authz_hook", &"SharedAuthzHook")
            .field("signing_keys", &self.signing_keys)
            .field("token_authority", &self.token_authority)
            .field("governance", &self.governance)
            .field("services", &self.services)
            .field("cache_epochs", &self.cache_epochs)
            .field(
                "schema_install_clean",
                &self.schema_install.boot().is_clean(),
            )
            .field("event_bridge", &self.event_bridge.status())
            .field("event_router", &self.event_router)
            .field("file_storage", &"FileStorage")
            .field("shutdown", &"ShutdownRequest")
            .field(
                "background_tasks_in_flight",
                &self.background_tasks.in_flight(),
            )
            .field("spawner", &"Spawner")
            .field("late_binders", &self.late_binders);
        debug.finish()
    }
}
