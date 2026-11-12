//! Composition of the [`DocumentsRuntime`] from the context's handles.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_extension::{DynBrainSink, DynNotificationSink, ExtensionRegistry};
use systemprompt_files::FilesConfig;
use systemprompt_files::documents::templates::load_catalog;
use systemprompt_files::issues::{IssueReportRepository, IssueService};
use systemprompt_governance::authz::{AccessControlRepository, SharedAuthzHook};
use systemprompt_models::authz::{SubjectAttributeSource, SubjectSourceHandle};
use systemprompt_traits::{BackgroundTasks, FileStorage};

use systemprompt_users::UserRepository;

use crate::documents::{
    DocumentsRuntime, DocumentsRuntimeParts, ExtensionAudiences, UserSubjectDirectory,
};
use crate::error::RuntimeResult;

pub(crate) struct DocumentsInputs<'a> {
    pub(crate) services_root: &'a std::path::Path,
    pub(crate) database: &'a DbPool,
    pub(crate) file_storage: &'a Arc<dyn FileStorage>,
    pub(crate) authz_hook: &'a SharedAuthzHook,
    pub(crate) subjects: &'a SubjectSourceHandle,
    pub(crate) event_router: &'a EventRouter,
    pub(crate) files_config: &'a FilesConfig,
    pub(crate) background_tasks: &'a BackgroundTasks,
    pub(crate) extensions: &'a ExtensionRegistry,
    pub(crate) notifications: &'a DynNotificationSink,
    pub(crate) brain: &'a DynBrainSink,
    pub(crate) audiences: &'a ExtensionAudiences,
}

pub(crate) fn build_documents(
    inputs: &DocumentsInputs<'_>,
) -> RuntimeResult<Arc<DocumentsRuntime>> {
    let catalog = load_catalog(inputs.services_root)?;
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
            inputs.audiences.clone(),
        )),
        event_router: inputs.event_router.clone(),
        notifications: Arc::clone(inputs.notifications),
        brain: Arc::clone(inputs.brain),
        catalog,
        config: inputs.files_config.documents_config().clone(),
        issues: Arc::new(IssueService::new(
            IssueReportRepository::new(inputs.database),
            Arc::clone(inputs.notifications),
            inputs.files_config.issues_config().clone(),
        )),
        background_tasks: inputs.background_tasks.clone(),
        unscoped_actor: None,
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
