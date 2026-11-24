//! Inventory vocabularies decode to exactly their stored spellings.

use systemprompt_marketplace::inventory::ManagedSourceKind;
use systemprompt_marketplace::managed::ReconciliationStatus;

#[test]
fn managed_source_kind_round_trips_every_stored_value() {
    for (text, kind) in [
        ("git", ManagedSourceKind::Git),
        ("local_tree", ManagedSourceKind::LocalTree),
        ("managed", ManagedSourceKind::Managed),
    ] {
        assert_eq!(
            serde_json::to_string(&kind).expect("serialize"),
            format!("\"{text}\"")
        );
        let decoded: ManagedSourceKind =
            serde_json::from_str(&format!("\"{text}\"")).expect("deserialize");
        assert_eq!(decoded, kind);
    }
    assert!(serde_json::from_str::<ManagedSourceKind>("\"svn\"").is_err());
}

#[test]
fn reconciliation_status_round_trips_every_stored_value() {
    for (text, status) in [
        ("open", ReconciliationStatus::Open),
        ("resolved", ReconciliationStatus::Resolved),
        (
            "withdrawal_proposed",
            ReconciliationStatus::WithdrawalProposed,
        ),
    ] {
        assert_eq!(
            serde_json::to_string(&status).expect("serialize"),
            format!("\"{text}\"")
        );
        let decoded: ReconciliationStatus =
            serde_json::from_str(&format!("\"{text}\"")).expect("deserialize");
        assert_eq!(decoded, status);
    }
    assert!(serde_json::from_str::<ReconciliationStatus>("\"closed\"").is_err());
}
