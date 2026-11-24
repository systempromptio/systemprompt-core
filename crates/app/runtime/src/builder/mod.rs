//! Builder that assembles an [`AppContext`] from profile + config state over
//! the admitted composition.
//!
//! The builder owns the bootstrap order: profile -> paths -> files -> database
//! -> logging -> schema install over the admitted members -> kernel seeds ->
//! [`Kernel::resolve`] -> the stages that read the resolved state (authz,
//! governance, repositories) -> the context. The kernel is seeded with plain
//! handles ([`kernel_state`]) before it resolves, so every member's provide
//! fn, slot and governance surface reads the kernel seeds; the authz hook and
//! policy engine are built from what it resolved, and the system admin is
//! resolved before it through the users member's admin-bootstrap view. The host
//! seeds the read half of the authz hook cell (`AuthzHookCell`) and binds it,
//! through the binder it kept out of the state, as soon as the hook exists, so
//! a member service that decides through the cell denies until then and decides
//! through the one hook from then on. The host seeds the read half of the
//! document store cell (`DocumentsHandlesSlot`) too and binds it, through its
//! binder, to the handles the `documents` member provides sealed, right after
//! the kernel resolves. A failing step propagates as
//! [`RuntimeError`](crate::error::RuntimeError); helpers are in [`assembly`].
//!
//! [`Kernel::resolve`]: crate::Kernel::resolve
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) mod assembly;
pub(crate) mod authz;
pub(crate) mod composition;
pub(crate) mod core_layer;
pub(crate) mod database;
mod debug_impl;
pub(crate) mod events;
pub(crate) mod governance;
pub(crate) mod kernel_binders;
pub(crate) mod kernel_seed;
pub(crate) mod kernel_state;
mod options;
use composition::{
    bind_secret_scanner, build_data_plane, build_domain_layer, load_identity, new_plugins,
    new_spawner, seeded_repositories,
};
use std::sync::Arc;

use systemprompt_database::MigrationConfig;
use systemprompt_events::EventRouter;
use systemprompt_extension::Admitted;
use systemprompt_files::documents::{DocumentsHandles, DocumentsHandlesSlot};
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_governance::contract::AuthzHooks;
use systemprompt_governance::policy::PolicyCatalog;
use systemprompt_loader::BootContext;
use systemprompt_manifest::services::root::ActiveProfilePath;
use systemprompt_traits::{BackgroundTasks, ConfigWatch, DatabaseHandle, SecretStore};

use crate::context::{
    AppContext, ConfigPlane, ContextPlanes, DataPlane, ShutdownRequest, Subsystems,
};
use crate::error::RuntimeResult;
use crate::kernel::Kernel;
use authz::{AuthzBuilt, authz_stage};
pub use core_layer::discover_vertex_models;
use core_layer::{CoreLayer, CoreOverrides, SchemaPolicy, init_core, install_schemas};
use events::event_plane;
pub(crate) use governance::load_governance;
use kernel_state::{ApiKernelSeed, KernelCoordination, KernelSeed, kernel_state};

/// Assembles an [`AppContext`], owning the bootstrap order described on the
/// module.
///
/// The admitted composition is fixed at construction. Schema installation is
/// off by default. Override it with the `with_*` methods before calling
/// [`build`](Self::build).
pub struct AppContextBuilder {
    boot: BootContext,
    admitted: Arc<Admitted>,
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

        let content = assembly::assemble_content(&config, &app_paths);
        let seeded = seeded_repositories(&database, &config.instance_id);
        let system_admin = load_identity(&self.boot, &self.admitted, &database).await?;
        let (router, bridge, listener_pool) =
            event_plane(self.event_router, &database, &config, &cache_epochs);
        drop(listener_pool);
        let background_tasks = self.background_tasks.unwrap_or_default();
        let secrets: Arc<dyn SecretStore> =
            Arc::<systemprompt_config::SecretsService>::clone(self.boot.secrets());
        let (documents_slot, documents_binder) = DocumentsHandlesSlot::unbound();
        let spawner = new_spawner()?;
        let token_authority: Arc<dyn systemprompt_models::auth::TokenAuthority> =
            Arc::new(systemprompt_security::DeploymentTokenAuthority::new(
                signing_keys.clone(),
                config.jwt_issuer.clone(),
            ));
        let (seed, binders) = kernel_state(ApiKernelSeed {
            base: KernelSeed {
                database: &database,
                app_paths: &app_paths,
                files_config: &files_config,
                profile,
                config: &config,
                cache_epochs: &cache_epochs,
                verification: signing_keys.keyring(),
                services: services.view(),
                services_root,
                profile_path: ActiveProfilePath {
                    path: self.boot.profile_path().to_path_buf(),
                },
                secrets,
                secret_lookup: Arc::<systemprompt_config::SecretsService>::clone(
                    self.boot.secrets(),
                ),
                connector_tokens: Arc::default(),
                file_storage: Arc::clone(&file_storage),
                background_tasks: background_tasks.clone(),
                coordination: KernelCoordination::over(&seeded.services, &seeded.coordination),
                system_admin: &system_admin,
                event_router: &router,
                documents: documents_slot,
                catalog_roles: composition::catalog_roles(&self.admitted),
                escalation: systemprompt_traits::EscalationDelivery::InProcess,
            },
            signing_keys: &signing_keys,
            token_authority: &token_authority,
            spawner: &spawner,
            child_secrets: Arc::<systemprompt_config::SecretsService>::clone(self.boot.secrets()),
            at_rest_pepper: systemprompt_traits::AtRestPepper::new(
                &self.boot.secrets().get().oauth_at_rest_pepper,
            ),
            content_config: content.content_config.as_ref(),
        });
        let handle: Arc<dyn DatabaseHandle> =
            Arc::<systemprompt_database::Database>::clone(&database);
        let subscriptions: Arc<dyn systemprompt_traits::events::AnalyticsSubscription> =
            Arc::new(systemprompt_events::AnalyticsSubscriptions::new(
                Arc::clone(&router.broadcasters().analytics),
            ));
        let mut kernel = Kernel::new(handle, background_tasks.clone(), seed)
            .with_handle::<systemprompt_extension::handles::AnalyticsBroadcaster>(
            subscriptions,
        );
        kernel.resolve(&self.admitted)?;
        let r = kernel.resolved();
        if let Some(documents) = r.optional::<Arc<DocumentsHandles>>() {
            documents_binder.bind(documents)?;
        }
        if let Some(dispatcher) = r.optional::<Arc<dyn systemprompt_traits::MessagingDispatcher>>()
        {
            binders.messaging.bind(dispatcher)?;
        } else {
            tracing::info!(
                "agent-http is not admitted; chat-platform messages are refused at dispatch"
            );
        }

        let (authz_hook, market) = authz_stage(
            profile,
            &database,
            &services,
            authz_hook_override,
            &AuthzBuilt {
                epochs: &cache_epochs,
                providers: &*r.get::<Arc<SubjectProviderSet>>()?,
                hooks: &*r.get::<Arc<AuthzHooks>>()?,
            },
        )?;
        binders.base.authz_hook.bind(Arc::clone(&authz_hook))?;
        let governance = ConfigWatch::new(
            load_governance(profile, &database, &*r.get::<Arc<PolicyCatalog>>()?).await?,
        );
        bind_secret_scanner(&governance.get(), binders.base.secret_scanner)?;
        let repositories = build_domain_layer(
            &seeded,
            &system_admin,
            r.optional::<systemprompt_traits::DynContextMaterializer>(),
        )
        .await?;
        let plugins = new_plugins(&market);
        let subsystems = Subsystems {
            logging: systemprompt_logging::LoggingRuntime::new(log_sink, Arc::clone(&system_admin)),
            system_admin,
            authz_hook,
            signing_keys,
            token_authority,
            governance,
            services,
            cache_epochs,
            schema_install: Arc::new(crate::SchemaState::from(schema_install)),
            event_bridge: Arc::new(bridge),
            event_router: router,
            file_storage,
            shutdown: self.shutdown.unwrap_or_default(),
            background_tasks,
            spawner,
            late_binders: kernel_binders::PendingBinders::holding(binders.late),
        };

        AppContext::from_parts(
            ContextPlanes {
                data: build_data_plane(database, repositories),
                cfg: ConfigPlane {
                    boot: self.boot,
                    config,
                    app_paths,
                    files_config,
                    route_classifier: content.route_classifier,
                    build: self.build,
                },
                plugins,
                subsystems,
            },
            Arc::new(kernel),
            self.admitted,
        )
    }
}
