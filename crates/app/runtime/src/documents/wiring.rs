//! Assembly of the document service from the runtime's shared handles.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::blob_store::BlobStore;
use systemprompt_files::documents::codecs::CodecRegistry;
use systemprompt_files::documents::repository::DocumentRepositories;
use systemprompt_files::documents::templates::{CatalogTemplate, TemplateRegistry};
use systemprompt_files::documents::{
    DocumentAccessRepositories, DocumentAccessService, DocumentEvents, DocumentService,
    DocumentServiceParts, DocumentsError, SubjectDirectory,
};
use systemprompt_governance::authz::{AccessControlRepository, SharedAuthzHook};
use systemprompt_models::authz::SubjectAttributeSource;
use systemprompt_traits::FileStorage;

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
    pub catalog: Vec<CatalogTemplate>,
    pub config: DocumentsConfig,
}

impl std::fmt::Debug for DocumentsRuntimeParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentsRuntimeParts")
            .field("catalog", &self.catalog.len())
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// The document store as one runtime handle: the repositories bundle and
/// the [`DocumentService`] built over it.
#[derive(Debug, Clone)]
pub struct DocumentsRuntime {
    repositories: Arc<DocumentRepositories>,
    service: Arc<DocumentService>,
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
            catalog,
            config,
        } = parts;
        let repositories = Arc::new(DocumentRepositories::new(&database));
        let access = Arc::new(DocumentAccessService::new(
            DocumentAccessRepositories {
                grants: Arc::clone(&repositories.grants),
                folders: Arc::clone(&repositories.folders),
                documents: Arc::clone(&repositories.documents),
                access_entities,
            },
            authz_hook,
            subjects,
            directory,
        ));
        let blobs = Arc::new(BlobStore::new(
            file_storage,
            Arc::clone(&repositories.blobs),
        ));
        let service = Arc::new(DocumentService::new(DocumentServiceParts {
            repos: Arc::clone(&repositories),
            blobs,
            codecs: Arc::new(CodecRegistry::discover()),
            templates: Arc::new(TemplateRegistry::discover(catalog)),
            access,
            events: Arc::new(DocumentEvents::new(Arc::new(event_router))),
            config,
        }));
        Self {
            repositories,
            service,
        }
    }

    #[must_use]
    pub const fn repositories(&self) -> &Arc<DocumentRepositories> {
        &self.repositories
    }

    #[must_use]
    pub const fn service(&self) -> &Arc<DocumentService> {
        &self.service
    }

    pub async fn reconcile_folder_entities(&self) -> Result<u64, DocumentsError> {
        self.service.access().reconcile_folder_entities().await
    }
}
