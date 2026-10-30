use systemprompt_files::documents::models::{
    LockHolder, LockHolderError, LockHolderKind, RevisionOrigin, RevisionOriginError,
};
use systemprompt_identifiers::{AgentName, DriveDeviceId, EditorId};
use systemprompt_models::documents::RevisionOriginKind;

const SESSION: &str = "0b9a3f8e-6c1d-4f5e-9a7b-2c3d4e5f6a7b";

#[test]
fn a_lock_holder_round_trips_through_its_two_columns() {
    let holders = [
        LockHolder::Bridge(DriveDeviceId::new("device-1")),
        LockHolder::Editor(EditorId::new("tab-9")),
        LockHolder::Agent(AgentName::new("claude-code")),
    ];
    for holder in holders {
        let parsed =
            LockHolder::from_columns(holder.kind().as_str(), holder.id_str()).expect("parse");
        assert_eq!(parsed, holder);
    }
}

#[test]
fn a_lock_holder_kind_fixes_the_type_of_its_id() {
    assert_eq!(
        LockHolder::Agent(AgentName::new("claude-code")).kind(),
        LockHolderKind::Agent
    );
    let bad_id = LockHolder::from_columns("agent", "not an agent").expect_err("space");
    assert!(matches!(
        bad_id,
        LockHolderError::Id {
            kind: LockHolderKind::Agent,
            ..
        }
    ));
    let empty = LockHolder::from_columns("bridge", "").expect_err("empty device");
    assert!(matches!(empty, LockHolderError::Id { .. }));
}

#[test]
fn an_unknown_lock_holder_kind_is_a_typed_error() {
    let error = LockHolder::from_columns("robot", "r2").expect_err("kind");
    assert!(matches!(error, LockHolderError::Kind(_)));
}

#[test]
fn a_revision_origin_round_trips_through_its_two_columns() {
    let origins = [
        RevisionOrigin::Api,
        RevisionOrigin::BridgeDevice(DriveDeviceId::new("device-1")),
        RevisionOrigin::EditSession(systemprompt_identifiers::DocumentEditSessionId::new(
            SESSION,
        )),
    ];
    for origin in origins {
        let parsed =
            RevisionOrigin::from_columns(origin.kind().as_str(), origin.id_str()).expect("parse");
        assert_eq!(parsed, origin);
    }
}

#[test]
fn a_revision_origin_pairs_its_kind_with_the_right_id_presence() {
    assert!(matches!(
        RevisionOrigin::from_columns("edit_session", None),
        Err(RevisionOriginError::MissingId {
            kind: RevisionOriginKind::EditSession
        })
    ));
    assert!(matches!(
        RevisionOrigin::from_columns("api", Some("x")),
        Err(RevisionOriginError::UnexpectedId {
            kind: RevisionOriginKind::Api
        })
    ));
    assert!(matches!(
        RevisionOrigin::from_columns("edit_session", Some("not-a-uuid")),
        Err(RevisionOriginError::Id { .. })
    ));
    assert!(matches!(
        RevisionOrigin::from_columns("carrier_pigeon", None),
        Err(RevisionOriginError::Kind(_))
    ));
}
