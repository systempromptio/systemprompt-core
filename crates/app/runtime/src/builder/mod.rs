//! Builder that assembles an [`AppContext`] from profile + config state.
//!
//! The builder owns the bootstrap order: profile -> paths -> files ->
//! database -> logging -> extensions -> ancillary services -> extension state
//! -> extension seams. Extension state is built once the context exists,
//! seeded with the kernel's handles ([`kernel_state`]) and honouring the
//! profile's disabled extensions, and bound to the authz hook's state handle
//! before anything serves, so
//! every [`Extension::provide`](systemprompt_extension::Extension::provide)
//! sees the database, config, event router and background tasks through its
//! `ExtensionContext`; it is built and its declared requirements validated
//! before routers, jobs and gateway guards are collected, so each of their
//! factories reads the finished state. The notifications service and the
//! brain are assembled by their members when the kernel builds their slots;
//! the builder only binds the document directory's audience resolvers from
//! the built notification seams. A
//! failing step propagates as [`RuntimeError`](crate::error::RuntimeError);
//! helpers live in [`assembly`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ai_service;
pub(crate) mod assembly;
pub(crate) mod authz;
pub(crate) mod composition;
pub(crate) mod core_layer;
pub(crate) mod database;
mod debug_impl;
pub(crate) mod documents;
pub(crate) mod events;
pub(crate) mod governance;
pub(crate) mod kernel_state;
mod options;
pub use composition::owner_reassignments;
use composition::{
    build_artifact_ingest, build_data_plane, build_domain_layer, new_plugins, new_spawner,
};

use std::sync::Arc;

use systemprompt_database::MigrationConfig;
use systemprompt_events::EventRouter;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_loader::BootContext;
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_traits::BackgroundTasks;
use systemprompt_users::UserService;

use crate::context::{AppContext, ConfigPlane, DataPlane, ShutdownRequest, Subsystems};
use crate::documents::ExtensionAudiences;
use crate::error::RuntimeResult;
pub use core_layer::discover_vertex_models;
use core_layer::{CoreLayer, CoreOverrides, SchemaPolicy, init_core};
use events::event_plane;
use governance::init_governed_extensions;
pub(crate) use governance::load_governance;

/// Assembles an [`AppContext`], owning the bootstrap order described on the
/// module.
///
/// All fields default to a no-op build: extensions are discovered via
/// inventory, schema installation is off, and the marketplace filter falls
/// back to the inventory-registered implementation (or an allow-all filter).
/// Override these with the `with_*` methods before calling
/// [`build`](Self::build).
pub struct AppContextBuilder {
    boot: BootContext,
    extension_registry: Option<ExtensionRegistry>,
    show_startup_warnings: bool,
    marketplace_filter: Option<Arc<dyn MarketplaceFilter>>,
    core: CoreOverrides,
    schema: SchemaPolicy,
    migration_config: MigrationConfig,
    shutdown: Option<ShutdownRequest>,
    background_tasks: Option<BackgroundTasks>,
    event_router: Option<EventRouter>,
    build: systemprompt_models::BuildIdentity,
}

impl AppContextBuilder {
    #[must_use]
    pub fn new(boot: BootContext) -> Self {
        Self {
            boot,
            extension_registry: None,
            show_startup_warnings: false,
            marketplace_filter: None,
            core: CoreOverrides::default(),
            schema: SchemaPolicy::default(),
            migration_config: MigrationConfig::default(),
            shutdown: None,
            background_tasks: None,
            event_router: None,
            build: systemprompt_models::BuildIdentity::core(),
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the one assembly sequence of the context; splitting it threads a dozen handles through helper signatures"
    )]
    pub async fn build(self) -> RuntimeResult<AppContext> {
        let CoreLayer {
            config,
            app_paths,
            files_config,
            database,
            authz_hook,
            signing_keys,
            file_storage,
            cache_epochs,
            log_sink,
            policy,
            services,
            market,
        } = init_core(&self.boot, self.core).await?;

        let (extensions, schema_install, governance) = init_governed_extensions(
            self.boot.profile(),
            self.extension_registry,
            self.schema,
            self.migration_config,
            &database,
        )
        .await?;

        let content = assembly::assemble_content_and_geoip(
            &config,
            &app_paths,
            &database,
            self.show_startup_warnings,
        )?;

        let (repositories, user_service, system_admin, mcp_registry) =
            build_domain_layer(&self.boot, &database, policy, &services).await?;

        let (ai_service, connector_tokens) = ai_service::build_ai_service(
            &self.boot,
            &database,
            &repositories,
            &mcp_registry,
            &services,
        );
        let plugins = new_plugins(extensions, mcp_registry, self.marketplace_filter, &market);
        let (router, bridge, listener_pool) =
            event_plane(self.event_router, &database, &config, &cache_epochs);
        let background_tasks = self.background_tasks.unwrap_or_default();
        drop(listener_pool);
        #[cfg(feature = "notifications")]
        let notification_sink =
            systemprompt_notifications::OutboxNotificationSink::over_write_pool(
                &database,
                &config.instance_id,
            );
        #[cfg(not(feature = "notifications"))]
        let notification_sink: systemprompt_extension::DynNotificationSink =
            Arc::new(crate::documents::NotificationsUnavailableSink);
        #[cfg(feature = "brain")]
        let brain_sink: systemprompt_extension::DynBrainSink = if self.boot.profile().brain.enabled
        {
            systemprompt_brain::OutboxBrainSink::over_write_pool(&database, &config.instance_id)
        } else {
            Arc::new(crate::documents::BrainUnavailableSink)
        };
        #[cfg(not(feature = "brain"))]
        let brain_sink: systemprompt_extension::DynBrainSink =
            Arc::new(crate::documents::BrainUnavailableSink);
        let audiences = ExtensionAudiences::default();
        let documents = documents::build_documents(&documents::DocumentsInputs {
            services_root: &self.boot.require_services_root().path,
            database: &database,
            file_storage: &file_storage,
            authz_hook: &authz_hook,
            subjects: &plugins.subjects,
            event_router: &router,
            files_config: &files_config,
            background_tasks: &background_tasks,
            extensions: &plugins.extension_registry,
            disabled_extensions: &self.boot.profile().extensions.disabled,
            notifications: &notification_sink,
            brain: &brain_sink,
            audiences: &audiences,
        })?;
        let subsystems = Subsystems {
            ai_service,
            artifact_ingest: build_artifact_ingest(&database, &governance.get()),
            logging: systemprompt_logging::LoggingRuntime::new(log_sink, Arc::clone(&system_admin)),
            system_admin,
            authz_hook,
            signing_keys,
            governance,
            services,
            cache_epochs,
            schema_install: Arc::new(crate::SchemaState::from(schema_install)),
            event_bridge: Arc::new(bridge),
            event_router: router,
            geoip_reader: content.geoip_reader,
            file_storage,
            shutdown: self.shutdown.unwrap_or_default(),
            background_tasks,
            spawner: new_spawner()?,
            publish_guard: Arc::default(),
            connector_tokens,
            documents,
        };

        let app = AppContext::from_parts(
            build_data_plane(
                database,
                content.fingerprint_repo,
                user_service,
                repositories,
            ),
            ConfigPlane {
                boot: self.boot,
                config,
                app_paths,
                files_config,
                content_config: content.content_config,
                route_classifier: content.route_classifier,
                build: self.build,
            },
            plugins,
            subsystems,
            &market.state,
        )?;
        #[cfg(feature = "notifications")]
        if let Some(seams) = app
            .extension_state
            .find::<Arc<systemprompt_notifications::NotificationSeams>>()
        {
            audiences.bind(seams.audiences.clone());
        }
        Ok(app)
    }
}
