//! Assembly of the document service from the runtime's shared handles.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_extension::{DynBrainSink, DynNotificationSink};
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::agents::{DocumentActor, registered_policy};
use systemprompt_files::documents::blob_store::BlobStore;
use systemprompt_files::documents::codecs::CodecRegistry;
use systemprompt_files::documents::repository::DocumentRepositories;
use systemprompt_files::documents::templates::{CatalogTemplate, TemplateRegistry};
use systemprompt_files::documents::{
    DocumentAccessRepositories, DocumentAccessService, DocumentEvents, DocumentService,
    DocumentServiceParts, DocumentsError, SubjectDirectory,
};
use systemprompt_files::issues::IssueService;
use systemprompt_files::uploads::{UploadService, UploadServiceParts};
use systemprompt_governance::authz::{AccessControlRepository, SharedAuthzHook};
use systemprompt_models::authz::SubjectAttributeSource;
use systemprompt_traits::{BackgroundTasks, FileStorage};

#[cfg(feature = "collab")]
use systemprompt_files::CollabConfig;
#[cfg(feature = "collab")]
use systemprompt_files::documents::collab::{CollabService, CollabTuning, OwnerConfig};
#[cfg(feature = "collab")]
use systemprompt_files::documents::engine::DocumentEngine;

/// The handles a [`DocumentsRuntime`] is assembled from.
#[derive(Clone)]
pub struct DocumentsRuntimeParts {
    pub database: DbPool,
    pub file_storage: Arc<dyn FileStorage>,
    pub access_entities: Arc<AccessControlRepository>,
    pub authz_hook: SharedAuthzHook,
    pub subjects: Arc<dyn SubjectAttributeSource>,
    pub directory: Arc<dyn SubjectDirectory>,
    pub event_router: EventRouter,
    pub notifications: DynNotificationSink,
    pub brain: DynBrainSink,
    pub catalog: Vec<CatalogTemplate>,
    pub config: DocumentsConfig,
    pub issues: Arc<IssueService>,
    pub background_tasks: BackgroundTasks,
    pub unscoped_actor: Option<DocumentActor>,
}

impl std::fmt::Debug for DocumentsRuntimeParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentsRuntimeParts")
            .field("catalog", &self.catalog.len())
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// The document store as one runtime handle.
///
/// It holds the repositories bundle, the [`DocumentService`] built over it
/// and, with the `collab` feature, the process's one live co-editing
/// service, whose timer runs on the runtime's background tasks. Beside it
/// sits the [`UploadService`] the resumable upload API runs on, over the
/// same blob store, and the [`IssueService`] whose report attachments are
/// that service's uploads.
#[derive(Debug, Clone)]
pub struct DocumentsRuntime {
    repositories: Arc<DocumentRepositories>,
    access: Arc<DocumentAccessService>,
    service: Arc<DocumentService>,
    uploads: Arc<UploadService>,
    issues: Arc<IssueService>,
    #[cfg(feature = "collab")]
    collab: Arc<CollabService>,
}

impl DocumentsRuntime {
    #[must_use]
    pub fn new(parts: DocumentsRuntimeParts) -> Self {
        let DocumentsRuntimeParts {
            database,
            file_storage,
            access_entities,
            authz_hook,
            subjects,
            directory,
            event_router,
            notifications,
            brain,
            catalog,
            config,
            issues,
            background_tasks,
            unscoped_actor,
        } = parts;
        let repositories = Arc::new(DocumentRepositories::new(&database));
        let access = access_service(
            &database,
            &repositories,
            (access_entities, authz_hook, subjects, directory),
            unscoped_actor,
        );
        let blobs = Arc::new(BlobStore::new(
            Arc::clone(&file_storage),
            Arc::clone(&repositories.blobs),
        ));
        #[cfg(feature = "collab")]
        let collab_config = config.collab;
        let uploads = Arc::new(UploadService::new(UploadServiceParts {
            storage: file_storage,
            uploads: Arc::clone(&repositories.uploads),
            blobs: Arc::clone(&blobs),
            max_upload_bytes: config.max_upload_bytes,
            limits: config.uploads,
            tasks: background_tasks.clone(),
        }));
        let service = Arc::new(DocumentService::new(DocumentServiceParts {
            repos: Arc::clone(&repositories),
            blobs,
            codecs: Arc::new(CodecRegistry::discover()),
            templates: Arc::new(TemplateRegistry::discover(catalog)),
            access: Arc::clone(&access),
            events: Arc::new(DocumentEvents::new(Arc::new(event_router))),
            notifications,
            brain,
            config,
        }));
        service.attach_uploads(&uploads);
        #[cfg(feature = "collab")]
        let collab = {
            #[cfg(feature = "engine-loro")]
            let engine: Option<Arc<dyn DocumentEngine>> = Some(Arc::new(
                systemprompt_files::documents::engine::loro::LoroEngine,
            ));
            #[cfg(not(feature = "engine-loro"))]
            let engine: Option<Arc<dyn DocumentEngine>> = None;
            let mut built =
                CollabService::new(Arc::clone(&service), collab_tuning(&collab_config), engine)
                    .with_uploads(Arc::clone(&uploads));
            if collab_config.owner.enabled {
                built = built.with_owner(OwnerConfig::at(owner_address(&collab_config)));
            }
            let collab = Arc::new(built);
            service.attach_collab(&collab);
            collab.start(&background_tasks);
            collab
        };
        #[cfg(not(feature = "collab"))]
        drop(background_tasks);
        Self {
            repositories,
            access,
            service,
            uploads,
            issues,
            #[cfg(feature = "collab")]
            collab,
        }
    }

    #[cfg(feature = "collab")]
    #[must_use]
    pub const fn collab(&self) -> &Arc<CollabService> {
        &self.collab
    }

    #[must_use]
    pub const fn repositories(&self) -> &Arc<DocumentRepositories> {
        &self.repositories
    }

    #[must_use]
    pub const fn access(&self) -> &Arc<DocumentAccessService> {
        &self.access
    }

    #[must_use]
    pub const fn service(&self) -> &Arc<DocumentService> {
        &self.service
    }

    #[must_use]
    pub const fn uploads(&self) -> &Arc<UploadService> {
        &self.uploads
    }

    #[must_use]
    pub const fn issues(&self) -> &Arc<IssueService> {
        &self.issues
    }

    pub async fn reconcile_folder_entities(&self) -> Result<u64, DocumentsError> {
        self.service.access().reconcile_folder_entities().await
    }
}

#[cfg(feature = "collab")]
fn collab_tuning(config: &CollabConfig) -> CollabTuning {
    CollabTuning {
        snapshot_after: std::time::Duration::from_secs(u64::from(config.snapshot_after_secs)),
        snapshot_after_updates: config.snapshot_after_updates,
        ..CollabTuning::default()
    }
}

#[cfg(feature = "collab")]
fn owner_address(config: &CollabConfig) -> String {
    let port = config.owner.advertise_port;
    if let Ok(url) = std::env::var("SYSTEMPROMPT_COLLAB_ADVERTISE_URL") {
        if !url.trim().is_empty() {
            return url;
        }
    }
    match std::env::var("FLY_PRIVATE_IP") {
        Ok(ip) if ip.contains(':') => format!("http://[{ip}]:{port}"),
        Ok(ip) if !ip.is_empty() => format!("http://{ip}:{port}"),
        _ => format!("http://127.0.0.1:{port}"),
    }
}

type AccessCollaborators = (
    Arc<AccessControlRepository>,
    SharedAuthzHook,
    Arc<dyn SubjectAttributeSource>,
    Arc<dyn SubjectDirectory>,
);

fn access_service(
    database: &DbPool,
    repositories: &DocumentRepositories,
    (access_entities, authz_hook, subjects, directory): AccessCollaborators,
    unscoped_actor: Option<DocumentActor>,
) -> Arc<DocumentAccessService> {
    let access = DocumentAccessService::new(
        DocumentAccessRepositories {
            grants: Arc::clone(&repositories.grants),
            folders: Arc::clone(&repositories.folders),
            documents: Arc::clone(&repositories.documents),
            access_entities,
        },
        authz_hook,
        subjects,
        directory,
    )
    .with_agent_policy(registered_policy(database));
    Arc::new(match unscoped_actor {
        Some(actor) => access.with_unscoped_actor(actor),
        None => access,
    })
}
