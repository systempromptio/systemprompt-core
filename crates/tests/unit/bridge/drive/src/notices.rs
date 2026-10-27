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
    let notice =
        notice_for(DocumentProposalEvent::OPENED, &opened(&document), None).expect("notice");
    assert_eq!(notice.kind, NoticeKind::ProposalOpened);
    assert_eq!(notice.document_id, document);
    assert_eq!(notice.from.as_str(), "agent-user");
    assert!(notice.proposal_id.is_some());
    assert_eq!(notice.detail.as_deref(), Some("tighten intro"));
}

fn shared_frame(document: &DocumentId, recipient: &str) -> String {
    format!(
        r#"{{"document_id":"{document}","document_name":"Plan.docx","recipient_user_id":"{recipient}","subject_kind":"role","subject_value":"team","level":"edit","by":"owner-1","shared_by":"Olivia Owner"}}"#
    )
}

#[test]
fn a_shared_frame_for_this_user_becomes_a_notice_naming_document_and_sharer() {
    let document = DocumentId::generate();
    let me = UserId::new("me");
    let notice =
        notice_for("document.shared", &shared_frame(&document, "me"), Some(&me)).expect("notice");
    assert_eq!(notice.kind, NoticeKind::Shared);
    assert_eq!(notice.from.as_str(), "owner-1");
    assert_eq!(notice.detail.as_deref(), Some("edit"));
    let text = notice_text(&DriveHandle::new().status(), &notice);
    assert_eq!(text.title_key, "drive-shared-title");
    assert!(
        text.args
            .iter()
            .any(|(k, v)| *k == "document" && v == "Plan.docx")
    );
    assert!(
        text.args
            .iter()
            .any(|(k, v)| *k == "from" && v == "Olivia Owner")
    );
}

#[test]
fn a_shared_frame_for_another_user_makes_no_notice() {
    let document = DocumentId::generate();
    let me = UserId::new("me");
    assert!(
        notice_for(
            "document.shared",
            &shared_frame(&document, "someone-else"),
            Some(&me)
        )
        .is_none()
    );
    assert!(notice_for("document.shared", &shared_frame(&document, "me"), None).is_none());
}

#[test]
fn other_and_malformed_frames_make_no_notice() {
    let document = DocumentId::generate();
    assert!(notice_for(DocumentProposalEvent::ACCEPTED, &opened(&document), None).is_none());
    assert!(notice_for("document.shared", "{\"nope\":1}", Some(&UserId::new("me"))).is_none());
}

#[test]
fn queued_notices_are_numbered_and_bounded_and_render_their_keys() {
    let handle = DriveHandle::new();
    let document = DocumentId::generate();
    for _ in 0..25 {
        let notice =
            notice_for(DocumentProposalEvent::OPENED, &opened(&document), None).expect("notice");
        handle.push_notice(notice);
    }
    let status = handle.status();
    assert_eq!(status.notices.len(), 20);
    assert_eq!(status.notices.first().map(|n| n.seq), Some(6));
    assert_eq!(status.notices.last().map(|n| n.seq), Some(25));
    let text = notice_text(&status, &status.notices[0].notice);
    assert_eq!(text.title_key, "drive-proposal-title");
    assert!(
        text.args
            .iter()
            .any(|(k, v)| *k == "document" && v == document.as_str())
    );
}
