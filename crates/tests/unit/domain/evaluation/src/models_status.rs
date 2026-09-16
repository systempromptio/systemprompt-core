//! Typed lifecycle statuses round-trip through their stored text and their
//! JSON wire form, and unknown stored values are rejected.

use systemprompt_evaluation::models::{
    AccountingStatus, ApprovalStatus, CampaignStatus, SuggestionStatus,
};

#[test]
fn campaign_status_round_trips_and_rejects_unknown_rows() {
    for status in [
        CampaignStatus::Active,
        CampaignStatus::Paused,
        CampaignStatus::Completed,
        CampaignStatus::Cancelled,
    ] {
        assert_eq!(CampaignStatus::parse(status.as_str()).unwrap(), status);
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            serde_json::Value::String(status.as_str().to_owned())
        );
    }
    assert!(CampaignStatus::parse("archived").is_err());
}

#[test]
fn approval_status_round_trips_and_rejects_unknown_rows() {
    for status in [
        ApprovalStatus::Pending,
        ApprovalStatus::Approved,
        ApprovalStatus::Denied,
        ApprovalStatus::Expired,
        ApprovalStatus::Consumed,
    ] {
        assert_eq!(ApprovalStatus::parse(status.as_str()).unwrap(), status);
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            serde_json::Value::String(status.as_str().to_owned())
        );
    }
    assert!(ApprovalStatus::parse("granted").is_err());
}

#[test]
fn accounting_status_wire_form_matches_the_stored_column() {
    for status in [
        AccountingStatus::Complete,
        AccountingStatus::Partial,
        AccountingStatus::Unknown,
    ] {
        let wire = serde_json::to_value(status).unwrap();
        assert_eq!(wire, serde_json::Value::String(status.as_str().to_owned()));
        assert_eq!(
            serde_json::from_value::<AccountingStatus>(wire).unwrap(),
            status
        );
    }
    assert!(serde_json::from_str::<AccountingStatus>("\"settled\"").is_err());
}

#[test]
fn suggestion_status_round_trips_and_rejects_unknown_rows() {
    for status in [
        SuggestionStatus::Draft,
        SuggestionStatus::Accepted,
        SuggestionStatus::Rejected,
    ] {
        assert_eq!(SuggestionStatus::parse(status.as_str()).unwrap(), status);
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            serde_json::Value::String(status.as_str().to_owned())
        );
    }
    assert!(SuggestionStatus::parse("applied").is_err());
}
