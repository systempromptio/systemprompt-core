use systemprompt_bridge::drive::state::ConflictNotice;
use systemprompt_bridge::drive::{DriveError, DriveState};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::documents::content_hash;

use crate::support::{Fixture, entry_for};

#[test]
fn state_round_trips_through_disk() {
    let fx = Fixture::new("http://127.0.0.1:9");
    fx.write("notes/plan.md", b"# plan");
    let mut state = DriveState::default();
    state.cursor = Some("cursor-7".to_owned());
    let path = fx.root.dir().join("notes/plan.md");
    state.entries.insert(
        "notes/plan.md".to_owned(),
        entry_for(
            &path,
            &DocumentId::generate(),
            &DocumentRevisionId::generate(),
            content_hash(b"# plan"),
        ),
    );
    state.conflicts.push(ConflictNotice {
        path: "notes/plan.md".to_owned(),
        sibling: "notes/plan (conflict from laptop).md".to_owned(),
        head_revision_id: DocumentRevisionId::generate(),
        detected_at: "2026-10-27T00:00:00Z".parse().expect("time"),
    });
    state.save(&fx.root.state_path()).expect("save");
    assert_eq!(
        DriveState::load(&fx.root.state_path()).expect("load"),
        state
    );
}

#[test]
fn missing_state_is_a_first_run() {
    let fx = Fixture::new("http://127.0.0.1:9");
    assert_eq!(
        DriveState::load(&fx.root.state_path()).expect("load"),
        DriveState::default()
    );
}

#[test]
fn corrupt_state_is_quarantined_and_reported() {
    let fx = Fixture::new("http://127.0.0.1:9");
    std::fs::write(fx.root.state_path(), b"{ not json").expect("seed");

    let err = DriveState::load(&fx.root.state_path()).expect_err("corrupt state must fail");
    let DriveError::StateCorrupt { quarantined, .. } = err else {
        panic!("expected StateCorrupt, got {err:?}");
    };
    assert!(
        !fx.root.state_path().exists(),
        "corrupt file is moved aside"
    );
    assert_eq!(
        std::fs::read(&quarantined).expect("quarantined copy"),
        b"{ not json"
    );
    assert_eq!(
        DriveState::load(&fx.root.state_path()).expect("next load"),
        DriveState::default()
    );
}
