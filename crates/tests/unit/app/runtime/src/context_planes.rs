//! Debug-format contracts of the four `AppContext` planes. Planes are
//! assembled directly (the `from_parts` embedder path) against the test
//! database; tests skip when no database is configured.

use std::sync::Arc;

use systemprompt_governance::authz::{AllowAllHook, NullAuditSink};
use systemprompt_models::RouteClassifier;
use systemprompt_runtime::{
    AppContext, ConfigPlane, ContextPlanes, DataPlane, Plugins, Subsystems,
};
use systemprompt_test_fixtures::unique_instance;

#[tokio::test]
async fn plane_debug_impls_flag_optional_members() {
    let test_instance = unique_instance();
    let pool = systemprompt_test_fixtures::test_db_pool().await;

    let data = DataPlane {
        service_repository: Arc::new(systemprompt_database::ServiceRepository::new(
            &pool,
            test_instance.clone(),
        )),
        coordination: Arc::new(systemprompt_database::CoordinationRepositories::new(
            &pool,
            test_instance.clone(),
        )),
    };
    let dbg = format!("{data:?}");
    assert!(dbg.contains("DataPlane"), "got: {dbg}");

    let cfg = ConfigPlane {
        route_classifier: Arc::new(RouteClassifier::new(None)),
        build: systemprompt_models::BuildIdentity::core(),
        boot: systemprompt_test_fixtures::ensure_test_bootstrap()
            .boot
            .clone(),
    };
    let dbg = format!("{cfg:?}");
    assert!(dbg.contains("ConfigPlane"), "got: {dbg}");

    let plugins = Plugins {
        subject_providers: Default::default(),
    };
    let dbg = format!("{plugins:?}");
    assert!(dbg.contains("Plugins"), "got: {dbg}");
    assert!(dbg.contains("subject_providers"), "got: {dbg}");

    let authz_hook: systemprompt_governance::authz::SharedAuthzHook =
        Arc::new(AllowAllHook::new(Arc::new(NullAuditSink)));
    let file_storage =
        systemprompt_storage::build_file_storage(systemprompt_storage::FileStorageBackend::Local {
            root: std::env::temp_dir(),
        });
    let event_router = systemprompt_events::EventRouter::local_only(Arc::new(
        systemprompt_events::EventBroadcasters::new(),
    ));
    let subsystems = Subsystems {
        logging: systemprompt_test_fixtures::fixture_logging_runtime(),
        authz_hook: Arc::clone(&authz_hook),
        governance: systemprompt_test_fixtures::default_governance_watch(),
        services: systemprompt_test_fixtures::fixture_services_state(),
        schema_install: Arc::new(systemprompt_runtime::SchemaState::from(
            systemprompt_database::SchemaInstallReport::default(),
        )),
        event_bridge: systemprompt_test_fixtures::fixture_event_bridge(&pool),
        shutdown: Default::default(),
        late_binders: Default::default(),
    };
    let dbg = format!("{subsystems:?}");
    assert!(dbg.contains("Subsystems"), "got: {dbg}");
    assert!(dbg.contains("event_bridge: NotStarted"), "got: {dbg}");
    assert!(dbg.contains("outbox: None"), "got: {dbg}");

    let (documents, _binder) = systemprompt_files::documents::DocumentsHandlesSlot::unbound();
    let seed = systemprompt_test_fixtures::app_context::fixture_kernel_seed(
        &pool,
        systemprompt_test_fixtures::app_context::FixtureSeedParts {
            file_storage,
            background_tasks: Default::default(),
            event_router,
            documents,
        },
    );
    let admitted = systemprompt_test_fixtures::fixture_admitted();
    let kernel = systemprompt_test_fixtures::resolved_kernel(&pool, &admitted, seed)
        .expect("the core composition resolves over the host seeds");
    AppContext::from_parts(
        ContextPlanes {
            data,
            cfg,
            plugins,
            subsystems,
        },
        kernel,
        admitted,
    )
    .expect("context over the resolved core kernel");
}
