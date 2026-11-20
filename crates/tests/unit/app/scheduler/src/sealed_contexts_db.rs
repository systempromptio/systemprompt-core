//! One seal, every runtime context: each context handed to member-authored
//! runtime code (the kernel's runtime state a runner service runs in, a job
//! context, a config-plane context, from either constructor) withholds every
//! type the kernel state seals. The checks enumerate the sealed set itself, so
//! a type sealed later is covered without changing them, and they fail the
//! moment one context is built from an unwithheld map.

use systemprompt_scheduler::repository::ConfigReconcileRepository;
use systemprompt_scheduler::services::ConfigReconcileService;
use systemprompt_test_fixtures::{fixture_actor, test_database_url};
use systemprompt_traits::{Dependencies, JobContext};

fn reachable(state: &Dependencies, context: &Dependencies) -> Vec<&'static str> {
    state
        .sealed_types()
        .filter(|(id, _)| context.contains_type(*id))
        .map(|(_, name)| name)
        .collect()
}

#[tokio::test]
async fn every_runtime_context_withholds_every_sealed_seed_of_the_kernel_state() {
    let (app, _schedule) = crate::test_extension::scheduled_app(&test_database_url()).await;
    let state = app.kernel().state();
    assert!(
        state.sealed_types().count() >= 6,
        "the kernel state seals its credential and boot-only seeds: {state:?}"
    );

    let runtime = app.kernel().runtime_state();
    assert_eq!(
        reachable(state, &runtime),
        Vec::<&str>::new(),
        "runner-service state"
    );

    let job = JobContext::new(fixture_actor(), state.clone());
    assert_eq!(
        reachable(state, job.dependencies()),
        Vec::<&str>::new(),
        "job context"
    );

    let planes = app.config_planes().expect("config planes");
    let built = ConfigReconcileService::new(
        planes,
        ConfigReconcileRepository::new(app.db_pool()),
        state.clone(),
    );
    assert_eq!(
        reachable(state, built.plane_dependencies()),
        Vec::<&str>::new(),
        "plane context over the unwithheld state"
    );

    let composed = ConfigReconcileService::from_app_context(
        &app,
        ConfigReconcileRepository::new(app.db_pool()),
    )
    .expect("plane service");
    assert_eq!(
        reachable(state, composed.plane_dependencies()),
        Vec::<&str>::new(),
        "plane context of the composed service"
    );
}
