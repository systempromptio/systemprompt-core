//! `AnalyticsExtension` is required because it owns the protective jobs. The
//! boot refusal of a profile that disables it is asserted on the full server
//! in `integration/api/server_mount_analytics.rs`.

use systemprompt_analytics::extension::AnalyticsExtension;
use systemprompt_extension::Extension;
use systemprompt_traits::Job;

#[test]
fn the_analytics_extension_is_required() {
    assert!(AnalyticsExtension.is_required());
}

#[test]
fn the_analytics_extension_owns_the_protective_jobs() {
    let owned: Vec<&str> = inventory_jobs()
        .into_iter()
        .filter(|(owner, _)| *owner == "analytics")
        .map(|(_, name)| name)
        .collect();
    for job in ["malicious_ip_blacklist", "behavioral_analysis"] {
        assert!(owned.contains(&job), "{job} in {owned:?}");
    }
}

fn inventory_jobs() -> Vec<(&'static str, &'static str)> {
    systemprompt_traits::registered_jobs()
        .map(|registration| (registration.owner, registration.job.name()))
        .collect()
}
