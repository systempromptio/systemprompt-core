//! Reviewing proposals: accept by compare-and-swap, accept by rebase onto
//! a moved head, the conflict refusal, reject with a note and authz.

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    Accepted, CommentPolicy, CommitOutcome, CommitRequest, ReviewRequest,
};
use systemprompt_identifiers::{DocumentId, DocumentProposalId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, ProposalStatus};

use crate::documents_proposals::{grant, memo, outline, propose, rewrite, texts};
use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, harness, person, trace,
};

fn review(proposal: &DocumentProposalId, note: Option<&str>) -> ReviewRequest {
    ReviewRequest {
        proposal_id: proposal.clone(),
        note: note.map(str::to_owned),
        reviewer: api_author(),
    }
}

async fn accept(
    h: &Harness,
    who: &AuthenticatedUser,
    proposal: &DocumentProposalId,
) -> Result<Accepted, DocumentsError> {
    h.service
        .accept_proposal(who, review(proposal, Some("looks good")), &trace())
        .await
}

async fn owner_edit(
    h: &Harness,
    owner: &AuthenticatedUser,
    document: &DocumentId,
    from: &str,
    to: &str,
) {
    let head = outline(h, owner, document, None).await;
    let outcome = h
        .service
        .commit(
            owner,
            CommitRequest {
                document_id: document.clone(),
                base_revision: None,
                body: CommitBody::Ops(vec![rewrite(&head, from, to)]),
                author: api_author(),
                message: None,
                lock_holder_id: None,
                comments: CommentPolicy::default(),
                op_id: None,
                media: Media::new(),
            },
            &trace(),
        )
        .await
        .expect("owner edit");
    assert!(!matches!(outcome, CommitOutcome::Conflict { .. }));
}

async fn setup(prefix: &str) -> (Harness, AuthenticatedUser, AuthenticatedUser, DocumentId) {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person(&format!("{prefix}-owner"));
    let proposer = person(&format!("{prefix}-proposer"));
    let (document, _) = memo(&h, &owner, None).await;
    grant(&h, &owner, &document, &proposer, AccessLevel::Comment).await;
    (h, owner, proposer, document)
}

#[tokio::test]
async fn accept_on_an_unmoved_head_makes_the_proposed_revision_the_head() {
    let (h, owner, proposer, document) = setup("cas").await;
    let before = outline(&h, &proposer, &document, None).await;
    let proposed = propose(
        &h,
        &proposer,
        &document,
        vec![rewrite(&before, "alpha", "ALPHA")],
    )
    .await
    .expect("propose");
    let accepted = accept(&h, &owner, &proposed.proposal.id)
        .await
        .expect("accept");
    assert!(!accepted.rebased);
    assert_eq!(accepted.revision.id, proposed.revision.id);
    assert_eq!(accepted.revision.author_user_id, proposer.id);
    assert_eq!(accepted.proposal.status, ProposalStatus::Accepted);
    assert_eq!(accepted.proposal.reviewer_user_id.as_ref(), Some(&owner.id));
    assert_eq!(accepted.proposal.note.as_deref(), Some("looks good"));
    let stored = h
        .repos
        .proposals
        .find(&proposed.proposal.id)
        .await
        .expect("find")
        .expect("proposal");
    assert_eq!(
        stored.landed_revision_id.as_ref(),
        Some(&proposed.revision.id)
    );
    let head = h
        .repos
        .revisions
        .head(&document)
        .await
        .expect("head")
        .expect("some head");
    assert_eq!(head.id, proposed.revision.id);
}

#[tokio::test]
async fn accept_after_the_head_moved_elsewhere_rebases_the_ops() {
    let (h, owner, proposer, document) = setup("rebase").await;
    let before = outline(&h, &proposer, &document, None).await;
    let proposed = propose(
        &h,
        &proposer,
        &document,
        vec![rewrite(&before, "alpha", "ALPHA")],
    )
    .await
    .expect("propose");
    owner_edit(&h, &owner, &document, "gamma", "GAMMA").await;
    let accepted = accept(&h, &owner, &proposed.proposal.id)
        .await
        .expect("accept");
    assert!(accepted.rebased);
    assert_ne!(accepted.revision.id, proposed.revision.id);
    assert_eq!(accepted.revision.author_user_id, owner.id);
    assert_eq!(
        accepted.revision.attribution.on_behalf_of.as_ref(),
        Some(&proposer.id)
    );
    let head = outline(&h, &owner, &document, None).await;
    assert_eq!(texts(&head), vec!["ALPHA", "beta", "GAMMA"]);
}

#[tokio::test]
async fn accept_refuses_when_the_same_node_changed_and_leaves_the_proposal_open() {
    let (h, owner, proposer, document) = setup("conflict").await;
    let before = outline(&h, &proposer, &document, None).await;
    let proposed = propose(
        &h,
        &proposer,
        &document,
        vec![rewrite(&before, "beta", "proposed")],
    )
    .await
    .expect("propose");
    owner_edit(&h, &owner, &document, "beta", "owner wins").await;
    let refused = accept(&h, &owner, &proposed.proposal.id).await;
    match refused {
        Err(DocumentsError::ProposalConflict {
            node, current_text, ..
        }) => {
            assert!(node.is_some());
            assert_eq!(current_text.as_deref(), Some("owner wins"));
        },
        other => panic!("expected proposal conflict, got {other:?}"),
    }
    let stored = h
        .repos
        .proposals
        .find(&proposed.proposal.id)
        .await
        .expect("find")
        .expect("proposal");
    assert_eq!(stored.status, ProposalStatus::Open);
    assert!(stored.reviewer_user_id.is_none());
}

#[tokio::test]
async fn reject_records_the_note_and_closes_the_proposal() {
    let (h, owner, proposer, document) = setup("reject").await;
    let before = outline(&h, &proposer, &document, None).await;
    let proposed = propose(
        &h,
        &proposer,
        &document,
        vec![rewrite(&before, "alpha", "nope")],
    )
    .await
    .expect("propose");
    let rejected = h
        .service
        .reject_proposal(
            &owner,
            review(&proposed.proposal.id, Some("off topic")),
            &trace(),
        )
        .await
        .expect("reject");
    assert_eq!(rejected.status, ProposalStatus::Rejected);
    assert_eq!(rejected.note.as_deref(), Some("off topic"));
    assert_eq!(rejected.reviewer_user_id.as_ref(), Some(&owner.id));
    let again = accept(&h, &owner, &proposed.proposal.id).await;
    assert!(
        matches!(
            again,
            Err(DocumentsError::ProposalClosed {
                status: ProposalStatus::Rejected,
                ..
            })
        ),
        "{again:?}"
    );
    let head = outline(&h, &owner, &document, None).await;
    assert_eq!(texts(&head), vec!["alpha", "beta", "gamma"]);
}

#[tokio::test]
async fn a_viewer_or_the_comment_only_proposer_cannot_accept() {
    let (h, owner, proposer, document) = setup("authz").await;
    let viewer = person("authz-viewer");
    grant(&h, &owner, &document, &viewer, AccessLevel::View).await;
    let before = outline(&h, &proposer, &document, None).await;
    let proposed = propose(
        &h,
        &proposer,
        &document,
        vec![rewrite(&before, "alpha", "x")],
    )
    .await
    .expect("propose");
    for who in [&viewer, &proposer] {
        let refused = accept(&h, who, &proposed.proposal.id).await;
        assert!(
            matches!(refused, Err(DocumentsError::Forbidden { .. })),
            "{refused:?}"
        );
    }
    let view = h
        .service
        .proposal_view(&viewer, &proposed.proposal.id, &trace())
        .await
        .expect("viewer reads the proposal");
    assert_eq!(texts(&view.base), vec!["alpha", "beta", "gamma"]);
    assert_eq!(texts(&view.proposed), vec!["x", "beta", "gamma"]);
}
