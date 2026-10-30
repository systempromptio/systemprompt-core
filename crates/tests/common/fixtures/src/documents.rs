use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_events::{EventBroadcasters, EventRouter};
use systemprompt_extension::DynNotificationSink;
use systemprompt_files::documents::agents::DocumentActor;
use systemprompt_files::DocumentsConfig;
use systemprompt_governance::authz::{
    AccessControlRepository, SharedAuthzHook, SubjectProviderSet,
};
use systemprompt_manifest::profile::NotificationsProfile;
use systemprompt_models::authz::SubjectAttributeSource;
use systemprompt_runtime::documents::{
    DocumentsRuntime, DocumentsRuntimeParts, ExtensionAudiences, UserSubjectDirectory,
};
use systemprompt_runtime::notifications::NotificationsHandle;
use systemprompt_runtime::{core_notifications, notification_sink, NotificationsInputs};
use systemprompt_traits::{BackgroundTasks, FileStorage};
use systemprompt_users::UserRepository;

pub fn fixture_notification_sink(pool: &DbPool) -> DynNotificationSink {
    notification_sink(pool, crate::unique_instance())
}

pub fn fixture_notifications(
    pool: &DbPool,
    event_router: &EventRouter,
    documents: &Arc<DocumentsRuntime>,
) -> Arc<NotificationsHandle> {
    core_notifications(&NotificationsInputs {
        database: pool,
        event_router,
        sink: &fixture_notification_sink(pool),
        documents,
        profile: &NotificationsProfile::default(),
        listener_pool: pool.write_pool().as_ref(),
    })
}

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
    fixture_documents_runtime_on(pool, file_storage, hook, subjects, BackgroundTasks::new())
}

pub fn fixture_documents_runtime_on(
    pool: &DbPool,
    file_storage: &Arc<dyn FileStorage>,
    hook: &SharedAuthzHook,
    subjects: Arc<dyn SubjectAttributeSource>,
    background_tasks: BackgroundTasks,
) -> Arc<DocumentsRuntime> {
    fixture_documents_runtime_configured(
        pool,
        file_storage,
        hook,
        subjects,
        background_tasks,
        DocumentsConfig::default(),
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "a fixture mirrors the runtime's constructor inputs one to one"
)]
pub fn fixture_documents_runtime_configured(
    pool: &DbPool,
    file_storage: &Arc<dyn FileStorage>,
    hook: &SharedAuthzHook,
    subjects: Arc<dyn SubjectAttributeSource>,
    background_tasks: BackgroundTasks,
    config: DocumentsConfig,
) -> Arc<DocumentsRuntime> {
    Arc::new(DocumentsRuntime::new(DocumentsRuntimeParts {
        database: Arc::clone(pool),
        file_storage: Arc::clone(file_storage),
        access_entities: Arc::new(AccessControlRepository::new(pool)),
        authz_hook: Arc::clone(hook),
        subjects,
        directory: Arc::new(UserSubjectDirectory::new(
            Arc::new(UserRepository::new(pool)),
            Vec::new(),
            ExtensionAudiences::default(),
        )),
        event_router: EventRouter::local_only(Arc::new(EventBroadcasters::new())),
        notifications: fixture_notification_sink(pool),
        brain: Arc::new(systemprompt_runtime::documents::BrainUnavailableSink),
        catalog: Vec::new(),
        config,
        issues: Arc::new(systemprompt_files::issues::IssueService::new(
            systemprompt_files::issues::IssueReportRepository::new(pool),
            fixture_notification_sink(pool),
            systemprompt_files::IssuesConfig {
                enabled: true,
                ..systemprompt_files::IssuesConfig::default()
            },
        )),
        background_tasks,
        unscoped_actor: Some(DocumentActor::Human),
    }))
}
