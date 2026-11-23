//! Composition of the [`DocumentsRuntime`] from the context's handles and the
//! resolved kernel state, and the binding of the host's
//! `DocumentsHandlesSlot` to it through the binder the host kept.
//!
//! The blob holders, the document seams, the agent access policy and the
//! shared audience set are slot and kernel outputs of the resolve, so the
//! document store is built after it; the files member's seams read the store
//! through the slot this module binds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_extension::{
    Admitted, AudienceResolvers, DynBrainSink, DynNotificationSink, Resolved,
};
use systemprompt_files::FilesConfig;
use systemprompt_files::contract::{AgentAccess, DocumentSeams};
use systemprompt_files::documents::repository::BlobHolders;
use systemprompt_files::documents::templates::load_catalog;
use systemprompt_files::documents::{DocumentsHandles, DocumentsHandlesBinder};
use systemprompt_files::issues::{IssueReportRepository, IssueService};
use systemprompt_governance::authz::{AccessControlRepository, SharedAuthzHook};
use systemprompt_models::authz::{SubjectAttributeSource, SubjectSourceHandle};
use systemprompt_traits::{BackgroundTasks, DynContextDirectory, FileStorage};

use systemprompt_users::UserRepository;

use crate::documents::{ContextsUnavailable, DocumentsRuntime, DocumentsRuntimeParts, UserSubjectDirectory};
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
    pub(crate) admitted: &'a Admitted,
    pub(crate) notifications: &'a DynNotificationSink,
    pub(crate) brain: &'a DynBrainSink,
    pub(crate) binder: &'a DocumentsHandlesBinder,
}

pub(crate) fn build_documents(
    inputs: &DocumentsInputs<'_>,
    r: &Resolved<'_>,
) -> RuntimeResult<Arc<DocumentsRuntime>> {
    let catalog = load_catalog(inputs.services_root)?;
    let subjects: Arc<dyn SubjectAttributeSource> = Arc::new(inputs.subjects.clone());
    let documents = Arc::new(DocumentsRuntime::new(DocumentsRuntimeParts {
        database: Arc::clone(inputs.database),
        file_storage: Arc::clone(inputs.file_storage),
        access_entities: Arc::new(AccessControlRepository::new(inputs.database)),
        authz_hook: Arc::clone(inputs.authz_hook),
        subjects,
        directory: Arc::new(UserSubjectDirectory::new(
            Arc::new(UserRepository::new(inputs.database)),
            catalog_roles(inputs.admitted),
            r.get::<Arc<AudienceResolvers>>()?,
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
        blob_holders: r.get::<Arc<BlobHolders>>()?,
        seams: r.get::<Arc<DocumentSeams>>()?,
        agent_access: r.get::<Arc<AgentAccess>>()?,
        contexts: r
            .optional::<DynContextDirectory>()
            .unwrap_or_else(|| Arc::new(ContextsUnavailable)),
    }));
    bind_documents(&documents, inputs.files_config, inputs.binder)?;
    Ok(documents)
}

fn bind_documents(
    documents: &DocumentsRuntime,
    files_config: &FilesConfig,
    binder: &DocumentsHandlesBinder,
) -> RuntimeResult<()> {
    let service = files_config
        .documents_config()
        .is_enabled()
        .then(|| Arc::clone(documents.service()));
    binder.bind(Arc::new(DocumentsHandles {
        access: Arc::clone(documents.access()),
        repositories: Arc::clone(documents.repositories()),
        issues: Arc::clone(documents.issues()),
        service,
    }))?;
    Ok(())
}

fn catalog_roles(admitted: &Admitted) -> Vec<String> {
    admitted
        .built()
        .members()
        .flat_map(|member| member.sink.roles())
        .map(|role| role.name.clone())
        .collect()
}
