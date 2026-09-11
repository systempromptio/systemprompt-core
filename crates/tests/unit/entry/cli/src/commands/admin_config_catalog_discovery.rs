//! Tests for the `admin config catalog discovery` row projection.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::admin::config::catalog::discovery_rows;
use systemprompt_models::services::DiscoveryReport;

fn report() -> DiscoveryReport {
    DiscoveryReport {
        discovered_priced: vec!["gemini-2.5-pro".to_owned()],
        discovered_unpriced: vec!["gemini-3.0-experimental".to_owned()],
        priced_not_published: vec!["openai/gpt-oss-20b-maas".to_owned()],
        explicit_wins: vec!["claude-opus-5".to_owned()],
        failed_publishers: vec!["meta".to_owned()],
        ran_at: "2026-09-11T04:30:00Z".to_owned(),
    }
}

#[test]
fn each_bucket_renders_one_row_with_its_own_state() {
    let rows = discovery_rows(&report());
    let pairs: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r.upstream_or_id.as_str(), r.state.as_str()))
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("gemini-2.5-pro", "served"),
            ("gemini-3.0-experimental", "unpriced"),
            ("openai/gpt-oss-20b-maas", "priced-not-published"),
            ("claude-opus-5", "explicit"),
        ]
    );
}

#[test]
fn an_empty_report_renders_no_rows() {
    assert!(discovery_rows(&DiscoveryReport::default()).is_empty());
}
