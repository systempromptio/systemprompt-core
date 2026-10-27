use systemprompt_bridge::drive::shares::{prune_synced, record};
use systemprompt_bridge::drive::{DriveHandle, DriveStatus};
use systemprompt_bridge::proxy::drive_signal::{DocumentNotice, NoticeKind};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, UserId};

use crate::support::Fixture;

fn shared(document: &DocumentId) -> DocumentNotice {
    DocumentNotice {
        kind: NoticeKind::Shared,
        document_id: document.clone(),
        from: UserId::new("owner-user"),
        proposal_id: None,
        detail: Some("edit".to_owned()),
    }
}

#[test]
fn drive_status_lists_shares_until_the_document_is_synced() {
    let fx = Fixture::new("http://127.0.0.1:9");
    let handle = DriveHandle::new();
    let document = DocumentId::generate();
    handle.push_notice(shared(&document));
    handle.push_notice(shared(&document));

    let mut state = fx.state();
    assert!(record(&mut state, handle.take_shares()));
    state.save(&fx.root.state_path()).expect("save");
    assert!(handle.take_shares().is_empty());

    let status = DriveStatus::from_disk(&fx.root).expect("status");
    assert_eq!(status.shares.len(), 1);
    assert_eq!(status.shares[0].document_id, document);
    assert_eq!(
        status.shares[0].shared_by.as_ref().map(UserId::as_str),
        Some("owner-user")
    );
    assert_eq!(status.shares[0].level.as_deref(), Some("edit"));

    fx.write("shared.md", b"from a colleague");
    fx.synced("shared.md", &document, &DocumentRevisionId::generate());
    let mut state = fx.state();
    assert!(prune_synced(&mut state));
    assert!(state.shares.is_empty());
}

#[test]
fn a_proposal_notice_is_not_a_share() {
    let handle = DriveHandle::new();
    let mut notice = shared(&DocumentId::generate());
    notice.kind = NoticeKind::ProposalOpened;
    handle.push_notice(notice);
    assert!(handle.take_shares().is_empty());
}
