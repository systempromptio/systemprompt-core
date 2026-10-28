//! Composition of the [`DocumentsRuntime`] from the context's handles.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_files::FilesConfig;
use systemprompt_files::documents::templates::load_catalog;
use systemprompt_governance::authz::{AccessControlRepository, SharedAuthzHook};
use systemprompt_loader::BootContext;
use systemprompt_models::authz::{SubjectAttributeSource, SubjectSourceHandle};
use systemprompt_traits::{BackgroundTasks, FileStorage};

use systemprompt_users::UserRepository;

use crate::documents::{DocumentsRuntime, DocumentsRuntimeParts, UserSubjectDirectory};
use crate::error::RuntimeResult;

pub(super) struct DocumentsInputs<'a> {
    pub(super) boot: &'a BootContext,
    pub(super) database: &'a DbPool,
    pub(super) file_storage: &'a Arc<dyn FileStorage>,
    pub(super) authz_hook: &'a SharedAuthzHook,
    pub(super) subjects: &'a SubjectSourceHandle,
    pub(super) event_router: &'a EventRouter,
    pub(super) files_config: &'a FilesConfig,
    pub(super) background_tasks: &'a BackgroundTasks,
    pub(super) extensions: &'a ExtensionRegistry,
}

pub(super) fn build_documents(
    inputs: &DocumentsInputs<'_>,
) -> RuntimeResult<Arc<DocumentsRuntime>> {
    let services_root = inputs.boot.require_services_root();
    let catalog = load_catalog(&services_root.path)?;
    let subjects: Arc<dyn SubjectAttributeSource> = Arc::new(inputs.subjects.clone());
    Ok(Arc::new(DocumentsRuntime::new(DocumentsRuntimeParts {
        database: Arc::clone(inputs.database),
        file_storage: Arc::clone(inputs.file_storage),
        access_entities: Arc::new(AccessControlRepository::new(inputs.database)),
        authz_hook: Arc::clone(inputs.authz_hook),
        subjects,
        directory: Arc::new(UserSubjectDirectory::new(
            Arc::new(UserRepository::new(inputs.database)),
            catalog_roles(inputs.extensions),
        )),
        event_router: inputs.event_router.clone(),
        catalog,
        config: *inputs.files_config.documents_config(),
        background_tasks: inputs.background_tasks.clone(),
    })))
}

fn catalog_roles(extensions: &ExtensionRegistry) -> Vec<String> {
    extensions
        .extensions()
        .iter()
        .flat_map(|extension| extension.roles())
        .map(|role| role.name)
        .collect()
}
