//! The proposal surface of the documents tools: `documents_edit` with
//! `mode: propose`, a comment-only caller's edit landing as a proposal,
//! and `documents_proposals` listing, accepting and rejecting.

use systemprompt_files::documents::models::GrantTarget;
use systemprompt_files::documents::service::ShareRequest;
use systemprompt_identifiers::{DocumentId, DocumentNodeId, TraceId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AccessLevel, ContentAs, EditMode, EditOp, NodeTarget, ProposalStatus,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::documents::mcp::{
    DocumentsMcpTools, EditHandler, EditInput, EditOutput, GuideHandler, GuideInput,
    ProposalAction, ProposalsHandler, ProposalsInput,
};

use crate::documents_mcp_support::{
    create_text, ctx_for, exec, harness, line_for, read_text, registered,
};

async fn edit(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    document: &DocumentId,
    (from, to): (&str, &str),
    mode: Option<EditMode>,
) -> EditOutput {
    let current = read_text(tools, ctx, document).await;
    let (node, hash) = line_for(&current, from);
    EditHandler { tools }
        .handle(
            EditInput {
                document_id: document.clone(),
                base_revision: None,
                mode,
                op_id: None,
                answering: None,
                allow_detach: false,
                ops: vec![EditOp::ReplaceText {
                    node: NodeTarget::Id(DocumentNodeId::new(node)),
                    expected_hash: Some(hash.to_owned()),
                    text: to.to_owned(),
                    content_as: ContentAs::Text,
                }],
                message: Some("suggestion".to_owned()),
                verbose: false,
            },
            ctx,
            &exec(),
        )
        .await
        .expect("edit")
        .0
}

async fn share(
    tools: &DocumentsMcpTools,
    owner: &AuthenticatedUser,
    document: &DocumentId,
    to: &AuthenticatedUser,
    level: AccessLevel,
) {
    tools
        .service()
        .share(
            owner,
            ShareRequest {
                target: GrantTarget::Document(document.clone()),
                subject_kind: "user".to_owned(),
                subject_value: to.id.to_string(),
                subject_display: None,
                level,
                valid_until: None,
                message: None,
            },
            &TraceId::generate(),
        )
        .await
        .expect("share");
}

fn texts(read: &systemprompt_runtime::documents::mcp::ReadOutput) -> Vec<String> {
    read.lines
        .iter()
        .filter_map(|line| line.split_once("| ").map(|(_, text)| text.to_owned()))
        .collect()
}

#[tokio::test]
async fn mode_propose_acknowledges_a_proposal_and_review_accepts_it() {
    let h = harness().await;
    let owner = registered("mcp-propose").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "one\ntwo\nthree").await;

    let ack = edit(
        &h.tools,
        &ctx,
        &document,
        ("two", "TWO"),
        Some(EditMode::Propose),
    )
    .await;
    let proposal = ack.proposal_id.clone().expect("proposal id");
    assert!(
        ack.written.iter().any(|line| line.ends_with("| TWO")),
        "{:?}",
        ack.written
    );
    let head = read_text(&h.tools, &ctx, &document).await;
    assert_eq!(texts(&head), vec!["one", "two", "three"]);

    let (listed, summary) = ProposalsHandler { tools: &h.tools }
        .handle(
            ProposalsInput {
                action: ProposalAction::List,
                document_id: Some(document.clone()),
                proposal_id: None,
                note: None,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("list");
    assert_eq!(listed.proposals.len(), 1);
    assert_eq!(listed.proposals[0].proposal_id, proposal);
    assert!(summary.contains("suggestion"), "{summary}");

    let accept = ProposalsInput {
        action: ProposalAction::Accept,
        document_id: None,
        proposal_id: Some(proposal.clone()),
        note: None,
    };
    let own = ProposalsHandler { tools: &h.tools }
        .handle(accept.clone(), &ctx, &exec())
        .await
        .expect_err("a proposer cannot accept their own proposal");
    let code = own
        .data
        .as_ref()
        .and_then(|d| d.get("code"))
        .and_then(|c| c.as_str());
    assert_eq!(code, Some("self_review"));

    let reviewer = registered("mcp-reviewer").await;
    share(&h.tools, &owner, &document, &reviewer, AccessLevel::Edit).await;
    let (output, _) = ProposalsHandler { tools: &h.tools }
        .handle(accept, &ctx_for(Some(&reviewer)), &exec())
        .await
        .expect("accept");
    let reviewed = output.reviewed.expect("reviewed");
    assert_eq!(reviewed.status, ProposalStatus::Accepted);
    assert_eq!(reviewed.revision_id.as_ref(), Some(&ack.revision_id));
    let head = read_text(&h.tools, &ctx, &document).await;
    assert_eq!(texts(&head), vec!["one", "TWO", "three"]);
}

#[tokio::test]
async fn a_comment_only_edit_proposes_and_a_viewer_review_is_refused() {
    let h = harness().await;
    let owner = registered("mcp-owner").await;
    let commenter = registered("mcp-commenter").await;
    let viewer = registered("mcp-viewer").await;
    let owner_ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &owner_ctx, "red\ngreen").await;
    share(
        &h.tools,
        &owner,
        &document,
        &commenter,
        AccessLevel::Comment,
    )
    .await;
    share(&h.tools, &owner, &document, &viewer, AccessLevel::View).await;

    let commenter_ctx = ctx_for(Some(&commenter));
    let ack = edit(&h.tools, &commenter_ctx, &document, ("green", "blue"), None).await;
    let proposal = ack.proposal_id.expect("comment-only edit proposes");

    let refused = ProposalsHandler { tools: &h.tools }
        .handle(
            ProposalsInput {
                action: ProposalAction::Accept,
                document_id: None,
                proposal_id: Some(proposal.clone()),
                note: None,
            },
            &ctx_for(Some(&viewer)),
            &exec(),
        )
        .await
        .expect_err("a viewer cannot accept");
    assert!(refused.message.contains("forbidden"), "{}", refused.message);

    let (output, _) = ProposalsHandler { tools: &h.tools }
        .handle(
            ProposalsInput {
                action: ProposalAction::Reject,
                document_id: None,
                proposal_id: Some(proposal),
                note: Some("keep green".to_owned()),
            },
            &owner_ctx,
            &exec(),
        )
        .await
        .expect("reject");
    let rejected = output.reviewed.expect("reviewed");
    assert_eq!(rejected.status, ProposalStatus::Rejected);
    assert!(rejected.revision_id.is_none());
}

#[tokio::test]
async fn the_proposals_guide_topic_names_the_review_actions() {
    let h = harness().await;
    let owner = registered("mcp-guide").await;
    let (_, text) = GuideHandler { tools: &h.tools }
        .handle(
            GuideInput {
                topics: vec!["proposals".to_owned()],
            },
            &ctx_for(Some(&owner)),
            &exec(),
        )
        .await
        .expect("guide");
    assert!(text.contains("action: accept"), "{text}");
    assert!(text.contains("proposal_conflict"), "{text}");
}
