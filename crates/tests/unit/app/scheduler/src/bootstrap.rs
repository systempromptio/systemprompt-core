use systemprompt_scheduler::SchedulerConfig;
use systemprompt_test_fixtures::fixture_system_admin;
use systemprompt_traits::Job;

#[test]
fn default_bootstrap_jobs_match_built_in_inventory_names() {
    let cfg = SchedulerConfig::with_system_admin(&fixture_system_admin("platform-admin"));
    assert_eq!(
        cfg.bootstrap_jobs,
        vec![
            "database_cleanup".to_string(),
            "cleanup_inactive_sessions".to_string(),
        ],
        "bootstrap_jobs default must list the two jobs that previously relied on the removed \
         Job::run_on_startup() trait method",
    );
}

#[test]
fn every_default_bootstrap_job_is_inventory_registered() {
    let cfg = SchedulerConfig::with_system_admin(&fixture_system_admin("platform-admin"));
    let registered: std::collections::HashSet<&'static str> = inventory::iter::<&'static dyn Job>
        .into_iter()
        .map(|j| j.name())
        .collect();

    for name in &cfg.bootstrap_jobs {
        assert!(
            registered.contains(name.as_str()),
            "default bootstrap job `{name}` is not registered via inventory; \
             SchedulerService::start would reject it with SchedulerError::UnknownJob",
        );
    }
}
