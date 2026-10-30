//! Builder that assembles an [`AppContext`] from profile + config state.
//!
//! The builder owns the bootstrap order: profile -> paths -> files ->
//! database -> logging -> extensions -> ancillary services. A failing step
//! propagates as [`RuntimeError`](crate::error::RuntimeError); helpers live
//! in [`assembly`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ai_service;
mod assembly;
mod authz;
#[cfg(feature = "brain")]
pub(crate) mod brain;
mod composition;
mod core_layer;
mod database;
mod debug_impl;
mod documents;
mod events;
mod governance;
#[cfg(feature = "notifications")]
pub(crate) mod notifications;
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

        let content = assembly::assemble_content_analytics(
            &config,
            &app_paths,
            &database,
            self.show_startup_warnings,
        )?;

        let analytics = content.analytics_repositories;
        let (repositories, user_service, system_admin, mcp_registry) =
            build_domain_layer(&self.boot, &database, analytics, policy, &services).await?;

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
        #[cfg(feature = "notifications")]
        let notification_sink =
            notifications::notification_sink(&database, config.instance_id.clone());
        #[cfg(not(feature = "notifications"))]
        let notification_sink: systemprompt_extension::DynNotificationSink =
            Arc::new(crate::documents::NotificationsUnavailableSink);
        #[cfg(not(feature = "notifications"))]
        drop(listener_pool);
        #[cfg(feature = "brain")]
        let brain_core = brain::BrainCore::new(&brain::BrainInputs {
            database: &database,
            instance_id: &config.instance_id,
            listener_pool: &listener_pool,
            notifications: &notification_sink,
            profile: &self.boot.profile().brain,
        });
        #[cfg(feature = "brain")]
        let brain_sink = brain::brain_sink(brain_core.as_ref());
        #[cfg(not(feature = "brain"))]
        let brain_sink: systemprompt_extension::DynBrainSink =
            Arc::new(crate::documents::BrainUnavailableSink);
        let audiences = ExtensionAudiences::default();
        let documents = documents::build_documents(&documents::DocumentsInputs {
            boot: &self.boot,
            database: &database,
            file_storage: &file_storage,
            authz_hook: &authz_hook,
            subjects: &plugins.subjects,
            event_router: &router,
            files_config: &files_config,
            background_tasks: &background_tasks,
            extensions: &plugins.extension_registry,
            notifications: &notification_sink,
            brain: &brain_sink,
            audiences: &audiences,
        })?;
        #[cfg(feature = "notifications")]
        let notifications_core =
            notifications::NotificationsCore::new(&notifications::NotificationsInputs {
                database: &database,
                event_router: &router,
                sink: &notification_sink,
                listener_pool: &listener_pool,
                documents: &documents,
                profile: &self.boot.profile().notifications,
            });
        #[cfg(feature = "brain")]
        let brain_handle = brain_core
            .as_ref()
            .map(|core| core.assemble(&documents, brain::RecallSeams::default()));
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
            #[cfg(feature = "notifications")]
            notifications: notifications_core.assemble(notifications::ExtensionSeams::default()),
            #[cfg(feature = "brain")]
            brain: brain_handle,
        };

        let app = AppContext::from_parts(
            build_data_plane(
                database,
                content.analytics_service,
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
            },
            plugins,
            subsystems,
        );
        #[cfg(feature = "notifications")]
        let app = {
            let mut app = app;
            let seams = notifications::ExtensionSeams::discover(&app);
            audiences.bind(seams.audiences.clone());
            app.subsystems.notifications = notifications_core.assemble(seams);
            app
        };
        #[cfg(feature = "brain")]
        let app = {
            let mut app = app;
            let seams = brain::RecallSeams::discover(&app);
            app.subsystems.brain = brain_core
                .as_ref()
                .map(|core| core.assemble(&app.subsystems.documents, seams));
            app
        };
        Ok(app)
    }
}
