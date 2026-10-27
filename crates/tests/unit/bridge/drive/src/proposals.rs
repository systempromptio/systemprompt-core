use systemprompt_bridge::drive::DriveStatus;
use systemprompt_bridge::drive::proposals::refresh;
use systemprompt_identifiers::{DocumentId, DocumentProposalId, DocumentRevisionId};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::Fixture;

#[tokio::test]
async fn a_cycle_refresh_stores_open_proposals_with_their_paths_for_drive_status() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    fx.write("notes/plan.md", b"# plan");
    let document = DocumentId::generate();
    fx.synced("notes/plan.md", &document, &DocumentRevisionId::generate());
    let proposal = DocumentProposalId::generate();
    Mock::given(method("GET"))
        .and(path("/v1/bridge/drive/proposals"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "proposals": [{
                "proposal_id": proposal.as_str(),
                "document_id": document.as_str(),
                "proposer_id": "agent-user",
                "actor_kind": "agent",
                "message": "tighten intro",
                "created_at": "2026-10-27T00:00:00Z"
            }]
        })))
        .mount(&server)
        .await;

    let mut state = fx.state();
    assert!(refresh(fx.cycle(), &mut state).await);
    assert!(!refresh(fx.cycle(), &mut state).await);
    state.save(&fx.root.state_path()).expect("save");

    let status = DriveStatus::from_disk(&fx.root).expect("status");
    assert_eq!(status.proposals.len(), 1);
    assert_eq!(status.proposals[0].proposal_id, proposal);
    assert_eq!(status.proposals[0].path.as_deref(), Some("notes/plan.md"));
}

#[tokio::test]
async fn a_failed_fetch_keeps_the_last_list() {
    let server = MockServer::start().await;
    let fx = Fixture::new(&server.uri());
    Mock::given(method("GET"))
        .and(path("/v1/bridge/drive/proposals"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let mut state = fx.state();
    state.proposals.push(systemprompt_bridge::drive::OpenProposal {
        proposal_id: DocumentProposalId::generate(),
        document_id: DocumentId::generate(),
        path: None,
        proposer_id: systemprompt_identifiers::UserId::new("someone"),
        message: None,
        created_at: "2026-10-27T00:00:00Z".parse().expect("time"),
    });
    assert!(!refresh(fx.cycle(), &mut state).await);
    assert_eq!(state.proposals.len(), 1);
}
