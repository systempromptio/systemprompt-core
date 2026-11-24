//! What `AnalyticsExtension` contributes: its schemas, the protective and
//! housekeeping jobs, the retention purges of its tables, the IP-ban-store
//! requirement and the four values it provides. The required standing is the
//! reference composition's, asserted by the facade tests; the boot refusal of
//! a profile that disables it is asserted on the full server in
//! `integration/api/server_mount_analytics.rs`.

use std::any::TypeId;
use std::sync::Arc;

use systemprompt_analytics::AnalyticsService;
use systemprompt_analytics::extension::AnalyticsExtension;
use systemprompt_analytics::repository::AnalyticsRepositories;
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Contributions, Extension};
use systemprompt_identifiers::ExtensionId;
use systemprompt_traits::{
    DynContentPopularity, DynIpBanStore, DynRequestAnalytics, DynSessionBehaviourScorer,
};

fn contributed() -> ContributionSink {
    let mut sink = ContributionSink::default();
    let mut c = Contributions::__new(ExtensionId::new("analytics"), &mut sink);
    AnalyticsExtension
        .contribute(&mut c)
        .expect("the analytics member contributes");
    sink
}

fn job_names(sink: &ContributionSink) -> Vec<&'static str> {
    sink.jobs().iter().map(|job| job.job().name()).collect()
}

#[test]
fn the_analytics_member_contributes_every_analytics_job() {
    let sink = contributed();
    let mut names = job_names(&sink);
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "analytics_event_link_facts_rollup",
            "backfill_session_geo",
            "behavioral_analysis",
            "ghost_session_cleanup",
            "malicious_ip_blacklist",
            "no_js_cleanup",
        ]
    );
}

#[test]
fn the_analytics_member_owns_the_protective_jobs() {
    let sink = contributed();
    let names = job_names(&sink);
    for job in ["malicious_ip_blacklist", "behavioral_analysis"] {
        assert!(names.contains(&job), "{job} in {names:?}");
    }
}

#[test]
fn the_analytics_member_contributes_the_purges_of_its_tables() {
    let sink = contributed();
    assert_eq!(sink.retention().len(), 2, "{:?}", sink.retention());
}

#[test]
fn the_analytics_member_requires_the_ip_ban_store() {
    let sink = contributed();
    let required: Vec<&str> = sink.requires().iter().map(|r| r.type_name()).collect();
    assert_eq!(required, [std::any::type_name::<DynIpBanStore>()]);
}

#[test]
fn the_analytics_member_provides_the_request_write_popularity_repositories_service_and_scorer() {
    let sink = contributed();
    let provided: Vec<TypeId> = sink.provides().iter().map(|p| p.type_id()).collect();
    assert_eq!(
        provided,
        [
            TypeId::of::<DynRequestAnalytics>(),
            TypeId::of::<DynContentPopularity>(),
            TypeId::of::<Arc<AnalyticsRepositories>>(),
            TypeId::of::<Arc<AnalyticsService>>(),
            TypeId::of::<DynSessionBehaviourScorer>(),
        ]
    );
}

#[test]
fn the_analytics_member_declares_its_schemas() {
    let sink = contributed();
    let tables: Vec<&str> = sink
        .schemas()
        .iter()
        .filter_map(|schema| schema.table.as_deref())
        .collect();
    for table in [
        "engagement_events",
        "fingerprint_reputation",
        "tool_usage_rollup_hourly",
        "agent_task_rollup_hourly",
    ] {
        assert!(tables.contains(&table), "{table} in {tables:?}");
    }
}
