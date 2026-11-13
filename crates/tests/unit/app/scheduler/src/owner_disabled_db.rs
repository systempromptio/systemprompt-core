//! A manual run of a job whose owning extension the profile disables is
//! refused end to end: the context is built from a profile that lists the
//! owner in `extensions.disabled`, and the run reports the owner rather than
//! running the job or reporting it unknown.

use std::collections::HashMap;

use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::{ExtensionId, JobName};
use systemprompt_scheduler::JobExecutionService;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config, test_db_pool,
};
use systemprompt_traits::registered_jobs;

#[tokio::test]
async fn a_job_whose_owner_the_profile_disables_is_refused_by_name() {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let probe = ExtensionRegistry::discover().expect("extension registry");

    let (owner, ctx) = registered_jobs()
        .map(|registration| registration.owner)
        .filter(|owner| *owner != "scheduler")
        .find_map(|owner| {
            let disabled = [ExtensionId::new(owner)];
            probe.enabled_extensions(&disabled).ok()?;
            let ctx = fixture_app_context_with_extensions(
                &pool,
                fixture_config(&bootstrap.database_url),
                ExtensionRegistry::discover().ok()?,
                &disabled,
            )
            .ok()?;
            Some((owner, ctx))
        })
        .expect("a linked, optional extension that owns a job and can be disabled");

    assert!(
        ctx.profile()
            .extensions
            .disabled
            .iter()
            .any(|id| id.as_str() == owner),
        "the fixture profile lists {owner} as disabled"
    );

    let job = registered_jobs()
        .find(|registration| registration.owner == owner)
        .map(|registration| registration.job.name())
        .expect("the owner has a job");

    let service = JobExecutionService::new(
        ctx,
        systemprompt_manifest::SchedulerConfig::with_system_admin(),
    );
    let report = service.run_job(&JobName::new(job), &HashMap::new()).await;

    assert!(!report.success, "a disabled owner's job must not run");
    let message = report.message.unwrap_or_default();
    assert!(
        message.contains(owner) && message.contains("profile disables"),
        "the refusal names the disabled owner: {message}"
    );
}
