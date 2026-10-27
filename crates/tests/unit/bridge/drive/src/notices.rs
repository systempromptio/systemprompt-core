use systemprompt_bridge::drive::DriveHandle;
use systemprompt_bridge::drive::notice::notice_text;
use systemprompt_bridge::proxy::drive_signal::{NoticeKind, notice_for};
use systemprompt_identifiers::{DocumentId, DocumentProposalId, DocumentRevisionId, UserId};
use systemprompt_models::documents::{DocumentProposalEvent, ProposalStatus};

fn opened(document: &DocumentId) -> String {
    serde_json::to_string(&DocumentProposalEvent {
        document_id: document.clone(),
        proposal_id: DocumentProposalId::generate(),
        status: ProposalStatus::Open,
        proposer_id: UserId::new("agent-user"),
        base_revision_id: DocumentRevisionId::generate(),
        proposed_revision_id: DocumentRevisionId::generate(),
        reviewer_id: None,
        landed_revision_id: None,
        message: Some("tighten intro".to_owned()),
    })
    .expect("event json")
}

#[test]
fn a_proposal_opened_frame_becomes_a_proposal_notice() {
    let document = DocumentId::generate();
    let notice = notice_for(DocumentProposalEvent::OPENED, &opened(&document)).expect("notice");
    assert_eq!(notice.kind, NoticeKind::ProposalOpened);
    assert_eq!(notice.document_id, document);
    assert_eq!(notice.from.as_str(), "agent-user");
    assert!(notice.proposal_id.is_some());
    assert_eq!(notice.detail.as_deref(), Some("tighten intro"));
}

#[test]
fn a_shared_frame_becomes_a_shared_notice_naming_the_level() {
    let document = DocumentId::generate();
    let frame = format!(
        r#"{{"document_id":"{document}","subject_kind":"user","subject_value":"me","level":"edit","by":"owner-1"}}"#
    );
    let notice = notice_for("document.shared", &frame).expect("notice");
    assert_eq!(notice.kind, NoticeKind::Shared);
    assert_eq!(notice.from.as_str(), "owner-1");
    assert_eq!(notice.detail.as_deref(), Some("edit"));
}

#[test]
fn other_and_malformed_frames_make_no_notice() {
    let document = DocumentId::generate();
    assert!(notice_for(DocumentProposalEvent::ACCEPTED, &opened(&document)).is_none());
    assert!(notice_for("document.shared", "{\"nope\":1}").is_none());
}

#[test]
fn queued_notices_are_numbered_and_bounded_and_render_their_keys() {
    let handle = DriveHandle::new();
    let document = DocumentId::generate();
    for _ in 0..25 {
        let notice = notice_for(DocumentProposalEvent::OPENED, &opened(&document)).expect("notice");
        handle.push_notice(notice);
    }
    let status = handle.status();
    assert_eq!(status.notices.len(), 20);
    assert_eq!(status.notices.first().map(|n| n.seq), Some(6));
    assert_eq!(status.notices.last().map(|n| n.seq), Some(25));
    let text = notice_text(&status, &status.notices[0].notice);
    assert_eq!(text.title_key, "drive-proposal-title");
    assert!(text.args.iter().any(|(k, v)| *k == "document" && v == document.as_str()));
}
