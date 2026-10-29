use std::str::FromStr;

use systemprompt_models::issues::{IssueKind, IssueSeverity, IssueStatus};

#[test]
fn only_open_triaged_and_needs_info_are_active() {
    let active: Vec<IssueStatus> = IssueStatus::ALL
        .iter()
        .copied()
        .filter(|status| status.is_active())
        .collect();
    assert_eq!(
        active,
        vec![
            IssueStatus::Open,
            IssueStatus::Triaged,
            IssueStatus::NeedsInfo
        ]
    );
}

#[test]
fn every_status_round_trips_through_its_wire_name() {
    for status in IssueStatus::ALL {
        assert_eq!(IssueStatus::from_str(status.as_str()), Ok(*status));
        assert_eq!(status.to_string(), status.as_str());
    }
    assert_eq!(IssueStatus::NeedsInfo.as_str(), "needs_info");
}

#[test]
fn every_kind_and_severity_round_trips_through_its_wire_name() {
    for kind in IssueKind::ALL {
        assert_eq!(IssueKind::from_str(kind.as_str()), Ok(*kind));
    }
    for severity in IssueSeverity::ALL {
        assert_eq!(IssueSeverity::from_str(severity.as_str()), Ok(*severity));
    }
}

#[test]
fn an_unknown_value_is_refused_and_named() {
    let error = IssueStatus::from_str("Open").expect_err("case-sensitive");
    assert_eq!(error.kind, "issue status");
    assert_eq!(error.value, "Open");
    assert!(IssueKind::from_str("question").is_err());
    assert!(IssueSeverity::from_str("urgent").is_err());
}

#[test]
fn the_wire_names_match_serde() {
    assert_eq!(
        serde_json::to_value(IssueStatus::NeedsInfo).expect("serialize"),
        serde_json::json!("needs_info")
    );
    assert_eq!(
        serde_json::from_value::<IssueSeverity>(serde_json::json!("critical")).expect("parse"),
        IssueSeverity::Critical
    );
}
