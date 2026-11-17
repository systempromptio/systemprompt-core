use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_events::{EventBroadcasters, EventRouter};
use systemprompt_extension::prelude::Dependencies;
use systemprompt_extension::{
    DynNotificationSink, NotificationAudienceResolver, ResourceVisibility, Slot,
};
use systemprompt_files::contract::{AgentAccess, DocumentSeams};
use systemprompt_files::documents::agents::DocumentActor;
use systemprompt_files::documents::repository::BlobHolders;
use systemprompt_files::documents::{DocumentVisibility, DocumentsHandles, DocumentsHandlesSlot};
use systemprompt_files::issues::IssueVisibility;
use systemprompt_files::DocumentsConfig;
use systemprompt_governance::authz::{
    AccessControlRepository, SharedAuthzHook, SubjectProviderSet,
};
use systemprompt_manifest::profile::NotificationsProfile;
use systemprompt_models::authz::SubjectAttributeSource;
use systemprompt_notifications::service::EscalationDelivery;
use systemprompt_notifications::{
    NotificationsConsumerSlot, NotificationsService, NotificationsServiceSlot, NotificationsSlot,
    OutboxNotificationSink,
};
use systemprompt_runtime::documents::{
    DocumentsRuntime, DocumentsRuntimeParts, UserSubjectDirectory,
};
use systemprompt_runtime::AppContext;
use systemprompt_traits::core_seam::{self, RoleAudience};
use systemprompt_traits::{BackgroundTasks, CoreSeam, FileStorage};
use systemprompt_users::{UserRepository, UserRoleAudienceResolver};

pub fn fixture_notification_sink(pool: &DbPool) -> DynNotificationSink {
    OutboxNotificationSink::over_write_pool(pool, &crate::unique_instance())
}

pub fn fixture_notifications(
    pool: &DbPool,
    event_router: &EventRouter,
    documents: &Arc<DocumentsRuntime>,
) -> Arc<NotificationsService> {
    let roles: Arc<dyn NotificationAudienceResolver> = Arc::new(UserRoleAudienceResolver::new(
        Arc::new(UserRepository::new(pool)),
    ));
    let document_visibility: Arc<dyn ResourceVisibility> = Arc::new(DocumentVisibility::new(
        Arc::clone(documents.access()),
        Arc::clone(documents.repositories()),
    ));
    let issue_visibility: Arc<dyn ResourceVisibility> = Arc::new(IssueVisibility::new(
        Arc::clone(documents.issues()),
        Arc::clone(&roles),
    ));
    let service = NotificationsServiceSlot::default();
    let mut seed = Dependencies::new()
        .with(Arc::clone(pool))
        .with(Arc::new(event_router.clone()))
        .with(Arc::new(NotificationsProfile::default()))
        .with(EscalationDelivery::InProcess)
        .with(fixture_notification_sink(pool))
        .with(service.clone())
        .with(NotificationsConsumerSlot::default())
        .with(CoreSeam::<dyn NotificationAudienceResolver, RoleAudience>::new(roles))
        .with(CoreSeam::<
            dyn ResourceVisibility,
            core_seam::DocumentVisibility,
        >::new(document_visibility))
        .with(
            CoreSeam::<dyn ResourceVisibility, core_seam::IssueVisibility>::new(issue_visibility),
        );
    let audiences = systemprompt_extension::AudienceResolvers::from_admitted(
        &systemprompt_extension::BuiltState::default(),
        &crate::app_context::fixture_kernel(pool, seed.clone()).resolved(),
    )
    .expect("the fixture audience set builds");
    seed.insert(Arc::new(audiences));
    let kernel = crate::app_context::fixture_kernel(pool, seed);
    NotificationsSlot::build(Vec::new(), &kernel.resolved())
        .expect("the notifications slot builds over the fixture seeds");
    Arc::clone(service.get().expect("the slot build binds the service"))
}

pub fn notifications_service(ctx: &AppContext) -> Arc<NotificationsService> {
    let state = systemprompt_extension::ExtensionContext::extension_state(ctx);
    let slot = state
        .get::<NotificationsServiceSlot>()
        .expect("the notifications extension provides its service slot");
    Arc::clone(
        slot.get()
            .expect("the fixture context binds the notification service"),
    )
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
    let (slot, binder) = DocumentsHandlesSlot::unbound();
    let roles: Arc<dyn NotificationAudienceResolver> = Arc::new(UserRoleAudienceResolver::new(
        Arc::new(UserRepository::new(pool)),
    ));
    let seed = Dependencies::new()
        .with(slot)
        .with(CoreSeam::<dyn NotificationAudienceResolver, RoleAudience>::new(roles));
    let admitted = crate::app_context::fixture_admitted_of(vec![Box::new(
        systemprompt_files::FilesExtension::default(),
    )]);
    let kernel = crate::composed::resolved_kernel(pool, &admitted, seed)
        .expect("the files member resolves over the fixture seeds");
    let r = kernel.resolved();
    let documents_enabled = config.is_enabled();
    let runtime = Arc::new(DocumentsRuntime::new(DocumentsRuntimeParts {
        database: Arc::clone(pool),
        file_storage: Arc::clone(file_storage),
        access_entities: Arc::new(AccessControlRepository::new(pool)),
        authz_hook: Arc::clone(hook),
        subjects,
        directory: Arc::new(UserSubjectDirectory::new(
            Arc::new(UserRepository::new(pool)),
            Vec::new(),
            Arc::new(systemprompt_extension::AudienceResolvers::default()),
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
        blob_holders: r
            .get::<Arc<BlobHolders>>()
            .expect("the files member builds its blob holders"),
        seams: r
            .get::<Arc<DocumentSeams>>()
            .expect("the files member builds its document seams"),
        agent_access: r
            .get::<Arc<AgentAccess>>()
            .expect("the files member builds its agent access"),
    }));
    let service = documents_enabled.then(|| Arc::clone(runtime.service()));
    binder
        .bind(Arc::new(DocumentsHandles {
            access: Arc::clone(runtime.access()),
            repositories: Arc::clone(runtime.repositories()),
            issues: Arc::clone(runtime.issues()),
            service,
        }))
        .expect("the fixture binds its own document store cell once");
    runtime
}
