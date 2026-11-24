//! Hand-written `Debug` implementations for the application context planes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{AppContext, ConfigPlane, DataPlane, Subsystems};

impl std::fmt::Debug for AppContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppContext")
            .field("kernel", &self.kernel)
            .field("admitted_members", &self.admitted.built().members().count())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for DataPlane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataPlane")
            .field("service_repository", &"ServiceRepository")
            .field("coordination", &"CoordinationRepositories")
            .finish()
    }
}

impl std::fmt::Debug for ConfigPlane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigPlane")
            .field("boot", &self.boot)
            .field("route_classifier", &"RouteClassifier")
            .field("build", &self.build)
            .finish()
    }
}

impl std::fmt::Debug for Subsystems {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subsystems")
            .field("logging", &self.logging)
            .field("authz_hook", &"SharedAuthzHook")
            .field("governance", &self.governance)
            .field("services", &self.services)
            .field(
                "schema_install_clean",
                &self.schema_install.boot().is_clean(),
            )
            .field("event_bridge", &self.event_bridge.status())
            .field("shutdown", &"ShutdownRequest")
            .finish_non_exhaustive()
    }
}
