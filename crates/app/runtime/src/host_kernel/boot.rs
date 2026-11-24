//! [`HostKernel::boot`]: the composition stages run over a child's granted
//! environment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_config::{GrantedSecrets, LoadedProfile};
use systemprompt_files::documents::{DocumentsHandles, DocumentsHandlesSlot};
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_governance::contract::AuthzHooks;
use systemprompt_governance::policy::PolicyCatalog;
use systemprompt_loader::{ConfigLoader, ServicesSourceBootstrap, ServicesState};
use systemprompt_models::subprocess::VERIFICATION_KEYS_ENV;
use systemprompt_security::jwt::VerifyingTokenAuthority;
use systemprompt_security::keys::{TokenVerificationKeys, VerificationKeyring};
use systemprompt_traits::{BackgroundTasks, CacheEpochs, DatabaseHandle, SecretStore};
use systemprompt_users::UserRepository;

use crate::builder::assembly::load_system_admin;
use crate::builder::authz::{AuthzBuilt, authz_stage};
use crate::builder::composition::bind_secret_scanner;
use crate::builder::core_layer::init_files_config;
use crate::builder::database::connect_database;
use crate::builder::events::event_plane;
use crate::builder::governance::load_governance;
use crate::builder::kernel_state::KernelSeed;
use crate::error::RuntimeError;
use crate::kernel::Kernel;

use super::{AdmitFn, HostBootError, HostKernel, HostParts, PROFILE_ENV};

impl HostKernel {
    pub async fn from_process_env(admit: AdmitFn) -> Result<HostParts, HostBootError> {
        Self::boot(|name| std::env::var(name).ok(), admit).await
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the one assembly sequence of a child host; splitting it threads a dozen handles through helper signatures"
    )]
    pub async fn boot(
        lookup: impl Fn(&str) -> Option<String> + Send + Sync,
        admit: AdmitFn,
    ) -> Result<HostParts, HostBootError> {
        let profile_path = lookup(PROFILE_ENV).ok_or(HostBootError::ProfileMissing)?;
        let loaded = LoadedProfile::from_path(Path::new(&profile_path))
            .map_err(|e| HostBootError::Profile(Box::new(e)))?;
        let profile = loaded.profile();
        let admitted = Arc::new(admit(profile).map_err(HostBootError::Admission)?);
        let secrets = Arc::new(
            GrantedSecrets::from_lookup(&lookup)
                .map_err(|e| HostBootError::Secrets(Box::new(e)))?,
        );
        let verification = verification_keyring(&lookup)?;

        let services_root = ServicesSourceBootstrap::read_composed(profile)
            .map_err(|e| HostBootError::ServicesRoot(Box::new(e)))?;
        let app_paths = Arc::new(
            AppPaths::from_profile(
                &profile.paths,
                profile.path_resolution(),
                Some(services_root.path.as_path()),
            )
            .map_err(RuntimeError::from)?,
        );
        let config = Arc::new(
            systemprompt_config::build_from_profile(
                &loaded,
                secrets.credentials(),
                Some(services_root.path.as_path()),
            )
            .map_err(|e| HostBootError::Config(Box::new(e)))?,
        );
        let files_config = init_files_config(profile, &app_paths)?;
        let services_config = ConfigLoader::for_profile(profile, &services_root.path)
            .load()
            .map_err(|e| HostBootError::Services(Box::new(e)))?;
        let services = ServicesState::from_config(profile, services_config)
            .map_err(|e| HostBootError::Services(Box::new(e)))?;

        let database = connect_database(profile, &config, systemprompt_database::PoolSeat::Child).await?;
        let instance_id = config.instance_id.clone();
        let log_sink = systemprompt_logging::init_logging(&database, Some(instance_id.clone()));
        let file_storage = crate::storage::init_file_storage(
            &profile.storage,
            &app_paths,
            &instance_id,
            &|name| secrets.get(name).map(str::to_owned),
        )
        .await?;
        let cache_epochs = CacheEpochs::new();
        let (event_router, _bridge) = event_plane(None, &database, &config, &cache_epochs);
        let background_tasks = BackgroundTasks::default();
        let user_repository = Arc::new(UserRepository::new(&database));
        let system_admin = load_system_admin(
            &config,
            &Arc::new(systemprompt_users::UserService::new(Arc::clone(
                &user_repository,
            ))),
        )
        .await?;
        let system_owner = system_admin.id().clone();

        let store: Arc<dyn SecretStore> = Arc::<GrantedSecrets>::clone(&secrets);
        let (documents_slot, documents_binder) = DocumentsHandlesSlot::unbound();
        let (seed, binders) = crate::base_kernel_state(KernelSeed {
            database: &database,
            app_paths: &app_paths,
            files_config: &files_config,
            profile,
            config: &config,
            cache_epochs: &cache_epochs,
            verification: &verification,
            services: services.view(),
            services_root: services_root.clone(),
            profile_path: systemprompt_manifest::services::root::ActiveProfilePath {
                path: std::path::PathBuf::from(&profile_path),
            },
            secrets: store,
            secret_lookup: Arc::<GrantedSecrets>::clone(&secrets),
            connector_tokens: Arc::default(),
            file_storage: Arc::clone(&file_storage),
            background_tasks: background_tasks.clone(),
            coordination: crate::KernelCoordination {
                instances: Arc::new(systemprompt_traits::UnavailableCoordination),
                leases: Arc::new(systemprompt_traits::UnavailableCoordination),
                locks: Arc::new(systemprompt_traits::UnavailableCoordination),
            },
            system_admin: &system_admin,
            event_router: &event_router,
            documents: documents_slot,
            catalog_roles: crate::builder::composition::catalog_roles(&admitted),
            escalation: systemprompt_traits::EscalationDelivery::Consumer,
        });
        let handle: Arc<dyn DatabaseHandle> =
            Arc::<systemprompt_database::Database>::clone(&database);
        let mut kernel = Kernel::new(handle, background_tasks.clone(), seed);
        kernel.resolve(&admitted).map_err(RuntimeError::from)?;
        let r = kernel.resolved();
        if let Some(documents) = r.optional::<Arc<DocumentsHandles>>() {
            documents_binder
                .bind(documents)
                .map_err(RuntimeError::from)?;
        }

        let (authz_hook, market) = authz_stage(
            profile,
            &database,
            &services,
            None,
            &AuthzBuilt {
                epochs: &cache_epochs,
                providers: &*r
                    .get::<Arc<SubjectProviderSet>>()
                    .map_err(RuntimeError::from)?,
                hooks: &*r.get::<Arc<AuthzHooks>>().map_err(RuntimeError::from)?,
            },
        )?;
        binders
            .authz_hook
            .bind(Arc::clone(&authz_hook))
            .map_err(RuntimeError::from)?;
        let governance = load_governance(
            profile,
            &database,
            &*r.get::<Arc<PolicyCatalog>>().map_err(RuntimeError::from)?,
        )
        .await
        .map_err(RuntimeError::from)?;
        bind_secret_scanner(&governance, binders.secret_scanner)?;

        let parts = HostParts {
            tokens: Arc::new(VerifyingTokenAuthority::new(
                verification.clone(),
                config.jwt_issuer.clone(),
            )),
            subjects: market.subjects.clone(),
            subject_providers: market.subject_providers,
            profile: Arc::clone(&loaded),
            services_root,
            config,
            app_paths,
            files_config,
            database,
            services,
            secrets,
            verification,
            authz_hook,
            background_tasks,
            log_sink,
            file_storage,
            kernel: Arc::new(kernel),
            admitted,
            system_owner,
        };
        Ok(parts)
    }
}

fn verification_keyring(
    lookup: &impl Fn(&str) -> Option<String>,
) -> Result<VerificationKeyring, HostBootError> {
    let raw = lookup(VERIFICATION_KEYS_ENV).ok_or(HostBootError::VerificationKeysMissing)?;
    let keys: TokenVerificationKeys =
        serde_json::from_str(&raw).map_err(HostBootError::VerificationKeysInvalid)?;
    VerificationKeyring::new(keys).map_err(HostBootError::VerificationKey)
}
