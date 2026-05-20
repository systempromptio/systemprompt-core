use systemprompt_scheduler::SchedulerConfig;
use systemprompt_traits::Job;

#[test]
fn default_bootstrap_jobs_match_built_in_inventory_names() {
    let cfg = SchedulerConfig::default();
    assert_eq!(
        cfg.bootstrap_jobs,
        vec![
            "database_cleanup".to_string(),
            "cleanup_inactive_sessions".to_string(),
        ],
        "bootstrap_jobs default must list the two jobs that previously \
         relied on the removed Job::run_on_startup() trait method",
    );
}

#[test]
fn every_default_bootstrap_job_is_inventory_registered() {
    let cfg = SchedulerConfig::default();
    let registered: std::collections::HashSet<&'static str> =
        inventory::iter::<&'static dyn Job>
            .into_iter()
            .map(|j| j.name())
            .collect();

    for name in &cfg.bootstrap_jobs {
        assert!(
            registered.contains(name.as_str()),
            "default bootstrap job `{name}` is not registered via inventory; \
             SchedulerService::run_bootstrap_jobs would warn-and-skip it",
        );
    }
}

#[test]
fn every_default_bootstrap_job_has_a_matching_job_config() {
    let cfg = SchedulerConfig::default();
    let configured: std::collections::HashSet<&str> =
        cfg.jobs.iter().map(|j| j.name.as_str()).collect();

    for name in &cfg.bootstrap_jobs {
        assert!(
            configured.contains(name.as_str()),
            "bootstrap job `{name}` has no JobConfig — SchedulerService::resolve_owners \
             would have nothing to resolve and the job would be skipped",
        );
    }
}
