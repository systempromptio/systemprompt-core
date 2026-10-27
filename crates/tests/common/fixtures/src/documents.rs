use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_events::{EventBroadcasters, EventRouter};
use systemprompt_files::DocumentsConfig;
use systemprompt_governance::authz::{
    AccessControlRepository, SharedAuthzHook, SubjectProviderSet,
};
use systemprompt_models::authz::SubjectAttributeSource;
use systemprompt_runtime::documents::{
    DocumentsRuntime, DocumentsRuntimeParts, UserSubjectDirectory,
};
use systemprompt_users::UserRepository;
use systemprompt_traits::FileStorage;

pub fn fixture_documents_runtime(
    pool: &DbPool,
    file_storage: &Arc<dyn FileStorage>,
    hook: &SharedAuthzHook,
) -> Arc<DocumentsRuntime> {
    fixture_documents_runtime_with(
        pool,
        file_storage,
        hook,
        Arc::new(SubjectProviderSet::default()),
    )
}

pub fn fixture_documents_runtime_with(
    pool: &DbPool,
    file_storage: &Arc<dyn FileStorage>,
    hook: &SharedAuthzHook,
    subjects: Arc<dyn SubjectAttributeSource>,
) -> Arc<DocumentsRuntime> {
    Arc::new(DocumentsRuntime::new(DocumentsRuntimeParts {
        database: Arc::clone(pool),
        file_storage: Arc::clone(file_storage),
        access_entities: Arc::new(AccessControlRepository::new(pool)),
        authz_hook: Arc::clone(hook),
        subjects,
        directory: Arc::new(UserSubjectDirectory::new(Arc::new(UserRepository::new(pool)))),
        event_router: EventRouter::local_only(Arc::new(EventBroadcasters::new())),
        catalog: Vec::new(),
        config: DocumentsConfig::default(),
    }))
}
