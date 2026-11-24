use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_documents::{DocumentsRuntime, DocumentsRuntimeParts, UserSubjectDirectory};
use systemprompt_events::{EventBroadcasters, EventRouter};
use systemprompt_extension::{
    DynNotificationSink, NotificationAudienceResolver, ResourceVisibility,
};
use systemprompt_files::DocumentsConfig;
use systemprompt_files::contract::{AgentAccess, DocumentSeams};
use systemprompt_files::documents::agents::DocumentActor;
use systemprompt_files::documents::repository::BlobHolders;
use systemprompt_files::documents::{DocumentVisibility, DocumentsHandlesSlot};
use systemprompt_files::issues::IssueVisibility;
use systemprompt_governance::authz::{
    AccessControlRepository, SharedAuthzHook, SubjectProviderSet,
};
use systemprompt_manifest::profile::NotificationsProfile;
use systemprompt_models::authz::SubjectAttributeSource;
use systemprompt_notifications::{
    NotificationsService, NotificationsServiceSlot, OutboxNotificationSink,
};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{BackgroundTasks, CoreSeam, FileStorage, core_seam};
use systemprompt_users::{UserRepository, UserRoleAudienceResolver};

pub fn fixture_notification_sink(pool: &DbPool) -> DynNotificationSink {
    OutboxNotificationSink::over_write_pool(pool, &crate::unique_instance())
}

// The notification service over the kernel seeds a host composes: the users
// member (its role audience) and the notifications member resolve over
// `fixture_kernel_seed` with the test's event router; the document and issue
// visibilities the files member provides in a host are read from the test's
// document store.
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
    let issue_visibility: Arc<dyn ResourceVisibility> =
        Arc::new(IssueVisibility::new(Arc::clone(documents.issues()), roles));
    let (documents_slot, _binder) = DocumentsHandlesSlot::unbound();
    let mut seed = crate::app_context::fixture_kernel_seed(
        pool,
        crate::app_context::FixtureSeedParts {
            file_storage: Arc::new(systemprompt_storage::LocalFileStorage::new(
                std::env::temp_dir(),
            )),
            background_tasks: BackgroundTasks::new(),
            event_router: event_router.clone(),
            documents: documents_slot,
        },
    );
    seed.insert(Arc::new(NotificationsProfile::default()));
    seed.insert(crate::unique_instance());
    seed.insert(CoreSeam::<
        dyn ResourceVisibility,
        core_seam::DocumentVisibility,
    >::new(document_visibility));
    seed.insert(
        CoreSeam::<dyn ResourceVisibility, core_seam::IssueVisibility>::new(issue_visibility),
    );
    let admitted = crate::app_context::fixture_admitted_of(vec![
        Box::new(systemprompt_users::UsersExtension),
        Box::new(systemprompt_notifications::NotificationsExtension),
    ]);
    let kernel = crate::composed::resolved_kernel(pool, &admitted, seed)
        .expect("the users and notifications members resolve over the fixture seeds");
    let slot = kernel
        .resolved()
        .get::<Arc<NotificationsServiceSlot>>()
        .expect("the notifications slot build returns its service");
    Arc::clone(slot.get().expect("the slot build binds the service"))
}

// The document store handles the context's documents member bound.
pub fn documents_handles(ctx: &AppContext) -> Arc<systemprompt_files::documents::DocumentsHandles> {
    let slot = ctx
        .kernel()
        .state()
        .find::<systemprompt_files::documents::DocumentsHandlesSlot>()
        .expect("the documents member provides the document store cell");
    Arc::clone(
        systemprompt_files::documents::bound(slot).expect("the documents member binds the cell"),
    )
}

// The link token view the documents member confers on files-http.
pub fn document_link_tokens(
    ctx: &AppContext,
) -> Arc<systemprompt_documents::tokens::DocumentLinkTokens> {
    Arc::clone(
        ctx.kernel()
            .state()
            .find::<Arc<systemprompt_documents::tokens::DocumentLinkTokens>>()
            .expect("the documents member provides the link token view"),
    )
}

pub fn notifications_service(ctx: &AppContext) -> Arc<NotificationsService> {
    let state = ctx.kernel().state();
    let slot = state
        .get::<Arc<NotificationsServiceSlot>>()
        .expect("the notifications slot build returns its service");
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

// The members a document store's provided values come from: the files
// member, and the users member whose role audience its issue visibility reads.
pub fn fixture_documents_admitted() -> Arc<systemprompt_extension::Admitted> {
    crate::app_context::fixture_admitted_of(vec![
        Box::new(systemprompt_users::UsersExtension),
        Box::new(systemprompt_files::FilesExtension),
    ])
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
    let event_router = EventRouter::local_only(Arc::new(EventBroadcasters::new()));
    let seed = crate::app_context::fixture_kernel_seed(
        pool,
        crate::app_context::FixtureSeedParts {
            file_storage: Arc::clone(file_storage),
            background_tasks: background_tasks.clone(),
            event_router: event_router.clone(),
            documents: slot,
        },
    );
    let kernel = crate::composed::resolved_kernel(pool, &fixture_documents_admitted(), seed)
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
        event_router,
        notifications: fixture_notification_sink(pool),
        brain: Arc::new(systemprompt_documents::BrainUnavailableSink),
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
        contexts: Arc::new(systemprompt_agent::services::ContextProviderService::new(
            systemprompt_agent::repository::ContextRepository::new(pool),
        )),
    }));
    binder
        .bind(Arc::new(runtime.handles(documents_enabled)))
        .expect("the fixture binds its own document store cell once");
    runtime
}
