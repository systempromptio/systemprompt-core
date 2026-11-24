//! The documents member's composed provides: the document store built from
//! the resolved state, and the two narrow token views it confers.
//!
//! The store is built after governance from slot outputs of the same resolve
//! the files member provides into (the blob holders, the document seams, the
//! agent access policy) and is provided sealed as its bound handles
//! (`Arc<DocumentsHandles>`): only kernel code reads it, and the host binds
//! it into the `DocumentsHandlesSlot` it seeded, through the binder it keeps
//! outside the state. Its access checks decide through [`LateAuthzHook`]
//! over the host's sealed `AuthzHookCell`, which the host binds once it has
//! built its authz hook; until then every check denies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_extension::prelude::{ProvideError, Resolved};
use systemprompt_extension::{AudienceResolvers, DynBrainSink, DynNotificationSink};
use systemprompt_files::FilesConfig;
use systemprompt_files::contract::{AgentAccess, DocumentSeams};
use systemprompt_files::documents::DocumentsHandles;
use systemprompt_files::documents::repository::BlobHolders;
use systemprompt_files::documents::templates::load_catalog;
use systemprompt_files::issues::{IssueReportRepository, IssueService};
use systemprompt_governance::authz::{
    AccessControlRepository, AuthzHookCell, LateAuthzHook, SubjectProviderSet,
};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_models::auth::{DynRequestAuthenticator, TokenAuthority};
use systemprompt_models::authz::{CatalogRoleNames, SubjectAttributeSource};
use systemprompt_traits::{BackgroundTasks, DynContextDirectory, DynDriveTokens, FileStorage};
use systemprompt_users::UserRepository;

use crate::tokens::{DocumentDriveTokens, DocumentLinkTokens};
use crate::{
    BrainUnavailableSink, ContextsUnavailable, DocumentsRuntime, DocumentsRuntimeParts,
    NotificationsUnavailableSink, UserSubjectDirectory,
};

fn failed(error: impl std::error::Error + Send + Sync + 'static) -> ProvideError {
    ProvideError::Failed(Box::new(error))
}

pub(crate) fn documents_handles(r: &Resolved<'_>) -> Result<Arc<DocumentsHandles>, ProvideError> {
    let database = r.get::<DbPool>()?;
    let files_config = r.get::<Arc<FilesConfig>>()?;
    let services_root = r.get::<ActiveServicesRoot>()?;
    let catalog = load_catalog(&services_root.path).map_err(failed)?;
    let providers = r.get::<Arc<SubjectProviderSet>>()?;
    let subjects: Arc<dyn SubjectAttributeSource> = Arc::new((*providers).clone());
    let notifications = r
        .optional::<DynNotificationSink>()
        .unwrap_or_else(|| Arc::new(NotificationsUnavailableSink));
    let brain_enabled = r
        .optional::<Arc<BrainProfile>>()
        .is_some_and(|profile| profile.enabled);
    let brain = brain_enabled
        .then(|| r.optional::<DynBrainSink>())
        .flatten()
        .unwrap_or_else(|| Arc::new(BrainUnavailableSink));
    let documents = Arc::new(DocumentsRuntime::new(DocumentsRuntimeParts {
        database: Arc::clone(&database),
        file_storage: r.get::<Arc<dyn FileStorage>>()?,
        access_entities: Arc::new(AccessControlRepository::new(&database)),
        authz_hook: Arc::new(LateAuthzHook::new(r.get::<AuthzHookCell>()?)),
        subjects,
        directory: Arc::new(UserSubjectDirectory::new(
            Arc::new(UserRepository::new(&database)),
            r.get::<Arc<CatalogRoleNames>>()?.names().to_vec(),
            r.get::<Arc<AudienceResolvers>>()?,
        )),
        event_router: (*r.get::<Arc<EventRouter>>()?).clone(),
        notifications: Arc::clone(&notifications),
        brain,
        catalog,
        config: files_config.documents_config().clone(),
        issues: Arc::new(IssueService::new(
            IssueReportRepository::new(&database),
            notifications,
            files_config.issues_config().clone(),
        )),
        background_tasks: r.get::<BackgroundTasks>()?,
        unscoped_actor: None,
        blob_holders: r.get::<Arc<BlobHolders>>()?,
        seams: r.get::<Arc<DocumentSeams>>()?,
        agent_access: r.get::<Arc<AgentAccess>>()?,
        contexts: r
            .optional::<DynContextDirectory>()
            .unwrap_or_else(|| Arc::new(ContextsUnavailable)),
    }));
    Ok(Arc::new(
        documents.handles(files_config.documents_config().is_enabled()),
    ))
}

pub(crate) fn mcp_users(
    r: &Resolved<'_>,
) -> Result<Arc<crate::mcp::DocumentsMcpUsers>, ProvideError> {
    let db = r.get::<DbPool>()?;
    Ok(Arc::new(crate::mcp::DocumentsMcpUsers(Arc::new(
        UserRepository::new(&db),
    ))))
}

pub(crate) fn link_tokens(r: &Resolved<'_>) -> Arc<DocumentLinkTokens> {
    Arc::new(DocumentLinkTokens::new(
        r.optional::<Arc<dyn TokenAuthority>>(),
    ))
}

pub(crate) fn drive_tokens(r: &Resolved<'_>) -> DynDriveTokens {
    Arc::new(DocumentDriveTokens::new(
        r.optional::<Arc<dyn TokenAuthority>>(),
        r.optional::<DynRequestAuthenticator>(),
    ))
}
