//! Proposals: who proposes (an agent under a `propose` folder, a
//! comment-only caller, an explicit `mode: propose`), what a proposed
//! revision is, supersession, the trash hook and the folder policy.

use std::time::Duration;

use systemprompt_events::Broadcaster;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::{GrantTarget, RevisionAttribution};
use systemprompt_files::documents::service::{
    Author, EditModeRequest, NewFolderRequest, ProposeRequest, Proposed, RevisionOrigin,
    ShareRequest,
};
use systemprompt_identifiers::{ConnectionId, DocumentFolderId, DocumentId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AccessLevel, ActorKind, AgentEditPolicy, ContentAs, EditMode, EditOp, NodeTarget, Projection,
    ProposalStatus,
};
use tokio::sync::mpsc;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, create, harness, person, trace, unique,
};

pub(crate) fn agent() -> Author {
    Author {
        actor_kind: ActorKind::Agent,
        origin: RevisionOrigin::Api,
        attribution: RevisionAttribution::default(),
    }
}

pub(crate) async fn memo(
    h: &Harness,
    owner: &AuthenticatedUser,
    folder: Option<&DocumentFolderId>,
) -> (DocumentId, DocumentRevisionId) {
    let mut request = create("blank-docx", folder);
    request.ops = vec![EditOp::AppendMarkdown {
        markdown: "alpha\n\nbeta\n\ngamma".to_owned(),
    }];
    let (document, revision) = h
        .service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create memo");
    (document.id, revision.id)
}

pub(crate) async fn outline(
    h: &Harness,
    who: &AuthenticatedUser,
    document: &DocumentId,
    revision: Option<&DocumentRevisionId>,
) -> Projection {
    h.service
        .project(who, document, revision, &trace())
        .await
        .expect("project")
        .1
}

pub(crate) fn texts(projection: &Projection) -> Vec<String> {
    projection
        .nodes()
        .filter(|node| node.children.is_empty())
        .filter_map(|node| node.text.clone())
        .filter(|text| !text.is_empty())
        .collect()
}

pub(crate) fn rewrite(projection: &Projection, from: &str, to: &str) -> EditOp {
    let node = projection
        .nodes()
        .find(|node| node.children.is_empty() && node.text.as_deref() == Some(from))
        .unwrap_or_else(|| panic!("no block {from}"));
    EditOp::ReplaceText {
        node: NodeTarget::Id(node.id.clone()),
        expected_hash: Some(node.hash8.clone()),
        text: to.to_owned(),
        content_as: ContentAs::Text,
    }
}

pub(crate) async fn grant(
    h: &Harness,
    owner: &AuthenticatedUser,
    document: &DocumentId,
    to: &AuthenticatedUser,
    level: AccessLevel,
) {
    h.service
        .share(
            owner,
            ShareRequest {
                target: GrantTarget::Document(document.clone()),
                subject_kind: "user".to_owned(),
                subject_value: to.id.to_string(),
                subject_display: None,
                level,
                valid_until: None,
            },
            &trace(),
        )
        .await
        .expect("share");
}

pub(crate) async fn propose(
    h: &Harness,
    who: &AuthenticatedUser,
    document: &DocumentId,
    ops: Vec<EditOp>,
) -> Result<Proposed, DocumentsError> {
    h.service
        .propose(
            who,
            ProposeRequest {
                document_id: document.clone(),
                base_revision: None,
                ops,
                author: agent(),
                message: Some("tighten wording".to_owned()),
            },
            &trace(),
        )
        .await
}

pub(crate) async fn mode(
    h: &Harness,
    who: &AuthenticatedUser,
    document: &DocumentId,
    requested: Option<EditMode>,
    actor: ActorKind,
) -> Result<EditMode, DocumentsError> {
    h.service
        .edit_mode(
            who,
            EditModeRequest {
                document,
                requested,
                actor,
            },
            &trace(),
        )
        .await
}

pub(crate) async fn folder(
    h: &Harness,
    owner: &AuthenticatedUser,
    parent: Option<&DocumentFolderId>,
) -> DocumentFolderId {
    h.service
        .create_folder(
            owner,
            NewFolderRequest {
                parent: parent.cloned(),
                name: unique("folder"),
            },
            &trace(),
        )
        .await
        .expect("folder")
        .id
}

#[tokio::test]
async fn an_agent_proposes_under_a_folder_that_inherits_propose() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("policy-owner");
    let parent = folder(&h, &owner, None).await;
    let child = folder(&h, &owner, Some(&parent)).await;
    let updated = h
        .service
        .set_folder_agent_edit_policy(&owner, &parent, AgentEditPolicy::Propose, &trace())
        .await
        .expect("set policy");
    assert_eq!(updated.agent_edit_policy, AgentEditPolicy::Propose);
    let (document, base) = memo(&h, &owner, Some(&child)).await;

    let agent_mode = mode(&h, &owner, &document, None, ActorKind::Agent).await;
    assert_eq!(agent_mode.expect("agent mode"), EditMode::Propose);
    let user_mode = mode(&h, &owner, &document, None, ActorKind::User).await;
    assert_eq!(user_mode.expect("user mode"), EditMode::Land);

    let before = outline(&h, &owner, &document, None).await;
    let proposed = propose(
        &h,
        &owner,
        &document,
        vec![rewrite(&before, "alpha", "ALPHA")],
    )
    .await
    .expect("propose");
    assert_eq!(proposed.proposal.status, ProposalStatus::Open);
    assert_eq!(proposed.proposal.base_revision_id, base);
    assert_eq!(proposed.proposal.actor_kind, ActorKind::Agent);
    assert_eq!(proposed.revision.parent_revision_id.as_ref(), Some(&base));
    let head = outline(&h, &owner, &document, None).await;
    assert_eq!(texts(&head), vec!["alpha", "beta", "gamma"]);
    let after = h
        .service
        .proposal_view(&owner, &proposed.proposal.id, &trace())
        .await
        .expect("proposal view")
        .proposed;
    assert_eq!(texts(&after), vec!["ALPHA", "beta", "gamma"]);
    let ids = |p: &Projection| p.nodes().map(|n| n.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&before), ids(&after));

    h.service
        .set_folder_agent_edit_policy(&owner, &parent, AgentEditPolicy::Direct, &trace())
        .await
        .expect("reset policy");
    let direct = mode(&h, &owner, &document, None, ActorKind::Agent).await;
    assert_eq!(direct.expect("direct"), EditMode::Land);
}

#[tokio::test]
async fn a_comment_only_caller_proposes_and_cannot_land() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("comment-owner");
    let commenter = person("commenter");
    let (document, _) = memo(&h, &owner, None).await;
    grant(&h, &owner, &document, &commenter, AccessLevel::Comment).await;

    let chosen = mode(&h, &commenter, &document, None, ActorKind::User).await;
    assert_eq!(chosen.expect("mode"), EditMode::Propose);
    let land = mode(
        &h,
        &commenter,
        &document,
        Some(EditMode::Land),
        ActorKind::User,
    )
    .await;
    assert!(
        matches!(land, Err(DocumentsError::Forbidden { .. })),
        "{land:?}"
    );

    let before = outline(&h, &commenter, &document, None).await;
    let proposed = propose(
        &h,
        &commenter,
        &document,
        vec![rewrite(&before, "beta", "BETA")],
    )
    .await
    .expect("comment-only propose");
    assert_eq!(proposed.proposal.proposer_user_id, commenter.id);
    assert_eq!(proposed.revision.author_user_id, commenter.id);
}

#[tokio::test]
async fn a_viewer_cannot_propose_and_an_editor_may_ask_for_propose() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("mode-owner");
    let viewer = person("viewer");
    let (document, _) = memo(&h, &owner, None).await;
    grant(&h, &owner, &document, &viewer, AccessLevel::View).await;

    let viewer_mode = mode(&h, &viewer, &document, None, ActorKind::User).await;
    assert!(matches!(viewer_mode, Err(DocumentsError::Forbidden { .. })));
    let explicit = mode(
        &h,
        &owner,
        &document,
        Some(EditMode::Propose),
        ActorKind::User,
    )
    .await;
    assert_eq!(explicit.expect("explicit"), EditMode::Propose);
}

#[tokio::test]
async fn a_newer_proposal_supersedes_the_open_one_and_events_reach_the_owner() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("supersede-owner");
    let editor = person("supersede-editor");
    let (document, _) = memo(&h, &owner, None).await;
    grant(&h, &owner, &document, &editor, AccessLevel::Edit).await;
    let (tx, mut rx) = mpsc::channel(32);
    assert!(
        h.broadcasters
            .agui
            .register(&owner.id, &ConnectionId::generate(), tx)
            .await
    );

    let before = outline(&h, &editor, &document, None).await;
    let first = propose(
        &h,
        &editor,
        &document,
        vec![rewrite(&before, "alpha", "one")],
    )
    .await
    .expect("first");
    let second = propose(
        &h,
        &editor,
        &document,
        vec![rewrite(&before, "alpha", "two")],
    )
    .await
    .expect("second");
    assert_eq!(second.superseded.len(), 1);
    assert_eq!(second.superseded[0].id, first.proposal.id);
    assert_eq!(second.superseded[0].status, ProposalStatus::Superseded);

    let open = h
        .service
        .list_proposals(&owner, &document, Some(ProposalStatus::Open), &trace())
        .await
        .expect("list open");
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, second.proposal.id);
    let mine = h.service.open_proposals_for(&owner).await.expect("mine");
    assert!(mine.iter().any(|p| p.id == second.proposal.id));

    let mut names = Vec::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
        names.push(format!("{event:?}"));
    }
    assert!(names.iter().any(|e| e.contains("document.proposal.opened")));
    assert!(
        names
            .iter()
            .any(|e| e.contains("document.proposal.superseded"))
    );
}

#[tokio::test]
async fn trashing_a_document_rejects_its_open_proposals() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("trash-owner");
    let (document, _) = memo(&h, &owner, None).await;
    let before = outline(&h, &owner, &document, None).await;
    let proposed = propose(&h, &owner, &document, vec![rewrite(&before, "gamma", "G")])
        .await
        .expect("propose");
    h.service
        .trash(&owner, &document, &trace())
        .await
        .expect("trash");
    let stored = h
        .repos
        .proposals
        .find(&proposed.proposal.id)
        .await
        .expect("find")
        .expect("proposal");
    assert_eq!(stored.status, ProposalStatus::Rejected);
    assert!(stored.note.is_some());
    assert!(
        h.service
            .open_proposals_for(&owner)
            .await
            .expect("open")
            .is_empty()
    );
}

#[tokio::test]
async fn only_share_access_sets_the_folder_policy() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("policy-guard");
    let stranger = person("policy-stranger");
    let target = folder(&h, &owner, None).await;
    let refused = h
        .service
        .set_folder_agent_edit_policy(&stranger, &target, AgentEditPolicy::Propose, &trace())
        .await;
    assert!(
        matches!(refused, Err(DocumentsError::NotFound { .. })),
        "{refused:?}"
    );
}
