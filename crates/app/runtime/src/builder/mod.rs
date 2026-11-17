//! Builder that assembles an [`AppContext`] from profile + config state over
//! the admitted composition.
//!
//! The builder owns the bootstrap order: profile -> paths -> files ->
//! database -> logging -> schema install over the admitted members ->
//! kernel seeds -> [`Kernel::resolve`] -> the stages that read the resolved
//! state (authz, governance, repositories, documents) -> the context. The
//! kernel is seeded with plain handles ([`kernel_state`]) before it resolves,
//! so every member's provide fn, slot and governance surface reads the
//! database, config, event router and the other kernel seeds; the authz hook,
//! publication policy, policy engine and document store are built from what
//! it resolved. The host seeds the read half of the document store cell
//! (`DocumentsHandlesSlot`) and binds it, through the binder it kept out of
//! the state, once the store exists, so the document seams other
//! members' slots appended read it from then on. A failing step propagates
//! as [`RuntimeError`](crate::error::RuntimeError); helpers live in
//! [`assembly`].
//!
//! [`Kernel::resolve`]: crate::Kernel::resolve
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
    build_artifact_ingest, build_data_plane, build_domain_layer, load_identity, new_plugins,
    new_spawner, seeded_repositories,
};

use std::sync::Arc;

use systemprompt_database::MigrationConfig;
use systemprompt_events::EventRouter;
use systemprompt_extension::Admitted;
use systemprompt_files::documents::DocumentsHandlesSlot;
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_governance::contract::AuthzHooks;
use systemprompt_governance::policy::PolicyCatalog;
use systemprompt_marketplace::contract::MarketplaceSeams;
use systemprompt_traits::{ConfigWatch, DatabaseHandle, SecretStore};
use systemprompt_loader::BootContext;
use systemprompt_marketplace::MarketplaceFilter;
use systemprompt_traits::BackgroundTasks;
use systemprompt_users::UserService;

use crate::context::{AppContext, ConfigPlane, DataPlane, ShutdownRequest, Subsystems};
use crate::error::RuntimeResult;
use crate::kernel::Kernel;
pub use core_layer::discover_vertex_models;
use authz::{AuthzBuilt, authz_stage};
use core_layer::{CoreLayer, CoreOverrides, SchemaPolicy, init_core, install_schemas};
use events::event_plane;
pub(crate) use governance::load_governance;
use kernel_state::{ApiKernelSeed, KernelSeed, kernel_state};

/// Assembles an [`AppContext`], owning the bootstrap order described on the
/// module.
///
/// The admitted composition is fixed at construction. Schema installation is
/// off by default, and the marketplace filter is the one the marketplace
/// seams built. Override these with the `with_*` methods before calling
/// [`build`](Self::build).
pub struct AppContextBuilder {
    boot: BootContext,
    admitted: Arc<Admitted>,
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
    pub fn new(boot: BootContext, admitted: Arc<Admitted>) -> Self {
        Self {
            boot,
            admitted,
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
            authz_hook_override,
            signing_keys,
            file_storage,
            cache_epochs,
            log_sink,
            services,
            services_root,
        } = init_core(&self.boot, self.core).await?;
        let profile = self.boot.profile();

        let schema_install = install_schemas(
            profile,
            &self.admitted,
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
        let seeded = seeded_repositories(&database, &self.boot.secrets().get())?;
        let (user_service, system_admin, mcp_registry) =
            load_identity(&self.boot, &database, &seeded.users, &services).await?;
        let (ai_service, connector_tokens) = ai_service::build_ai_service(
            &self.boot,
            &database,
            &seeded.ai,
            &mcp_registry,
            &services,
        );
        let (router, bridge, listener_pool) =
            event_plane(self.event_router, &database, &config, &cache_epochs);
        drop(listener_pool);
        let background_tasks = self.background_tasks.unwrap_or_default();

        let secrets: Arc<dyn SecretStore> =
            Arc::<systemprompt_config::SecretsService>::clone(self.boot.secrets());
        let (documents_slot, documents_binder) = DocumentsHandlesSlot::unbound();
        let seed = kernel_state(ApiKernelSeed {
            base: KernelSeed {
                database: &database,
                app_paths: &app_paths,
                files_config: &files_config,
                profile,
                config: &config,
                services: services.view(),
                services_root,
                secrets,
                system_admin: &system_admin,
                event_router: &router,
                documents: documents_slot,
                #[cfg(feature = "notifications")]
                escalation: systemprompt_notifications::service::EscalationDelivery::InProcess,
            },
            oauth: &seeded.oauth,
            ai: &seeded.ai,
            ai_service: ai_service.as_ref(),
            geoip: content.geoip_reader.as_ref(),
            content_config: content.content_config.as_ref(),
        });
        let handle: Arc<dyn DatabaseHandle> = database.clone();
        let subscriptions: Arc<dyn systemprompt_traits::events::AnalyticsSubscription> = Arc::new(
            systemprompt_events::AnalyticsSubscriptions::new(Arc::clone(&router.broadcasters().analytics)),
        );
        let mut kernel = Kernel::new(handle, background_tasks.clone(), seed)
            .with_handle::<systemprompt_extension::handles::AnalyticsBroadcaster>(subscriptions);
        kernel.resolve(&self.admitted)?;
        let r = kernel.resolved();

        let seams = r.get::<Arc<MarketplaceSeams>>()?;
        let (policy, authz_hook, market) = authz_stage(
            profile,
            &database,
            &services,
            authz_hook_override,
            &cache_epochs,
            &AuthzBuilt {
                providers: &*r.get::<Arc<SubjectProviderSet>>()?,
                hooks: &*r.get::<Arc<AuthzHooks>>()?,
                seams: &seams,
            },
        )?;
        let governance = ConfigWatch::new(
            load_governance(profile, &database, &*r.get::<Arc<PolicyCatalog>>()?).await?,
        );
        let repositories =
            build_domain_layer(&self.boot, &database, policy, &seeded, &system_admin).await?;
        let plugins = new_plugins(mcp_registry, self.marketplace_filter, &seams, &market);

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
        let brain_sink: systemprompt_extension::DynBrainSink = if profile.brain.enabled {
            systemprompt_brain::OutboxBrainSink::over_write_pool(&database, &config.instance_id)
        } else {
            Arc::new(crate::documents::BrainUnavailableSink)
        };
        #[cfg(not(feature = "brain"))]
        let brain_sink: systemprompt_extension::DynBrainSink =
            Arc::new(crate::documents::BrainUnavailableSink);
        let documents = documents::build_documents(
            &documents::DocumentsInputs {
                services_root: &self.boot.require_services_root().path,
                database: &database,
                file_storage: &file_storage,
                authz_hook: &authz_hook,
                subjects: &plugins.subjects,
                event_router: &router,
                files_config: &files_config,
                background_tasks: &background_tasks,
                admitted: &self.admitted,
                notifications: &notification_sink,
                brain: &brain_sink,
                binder: &documents_binder,
            },
            &r,
        )?;
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

        AppContext::from_parts(
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
            Arc::new(kernel),
            self.admitted,
        )
    }
}
