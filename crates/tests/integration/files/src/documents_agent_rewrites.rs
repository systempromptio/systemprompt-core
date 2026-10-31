use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::agents::{AgentCap, AgentCapability, AgentRefusal};
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommentPolicy, CommitOutcome, CommitRequest};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{ContentAs, EditOp, NodeTarget};

use crate::documents_agent_fixture::{agent_harness, agent_refusal, as_agent};
use crate::documents_service_fixture::{Harness, RoleDirectory, api_author, create, person, trace};

async fn lines(h: &Harness, owner: &AuthenticatedUser, count: usize) -> DocumentId {
    let mut request = create("blank-text", None);
    let text: String = (0..count).map(|n| format!("line {n}\n")).collect();
    request.ops = vec![EditOp::SetText { text }];
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

fn commit(id: &DocumentId, body: CommitBody) -> CommitRequest {
    CommitRequest {
        document_id: id.clone(),
        base_revision: None,
        body,
        author: api_author(),
        message: None,
        lock_holder: None,
        comments: CommentPolicy::detach(),
        op_id: None,
        media: Media::new(),
    }
}

async fn rewrite(
    h: &Harness,
    owner: &AuthenticatedUser,
    id: &DocumentId,
    which: &[usize],
) -> Result<CommitOutcome, DocumentsError> {
    let (_, projection) = h
        .service
        .project(owner, id, None, &trace())
        .await
        .expect("project");
    let ops = which
        .iter()
        .map(|n| {
            let node = projection
                .nodes()
                .find(|node| node.text.as_deref() == Some(&*format!("line {n}")))
                .expect("line");
            EditOp::ReplaceText {
                node: NodeTarget::Id(node.id.clone()),
                expected_hash: Some(node.hash8.clone()),
                text: format!("rewritten {n}"),
                content_as: ContentAs::Text,
            }
        })
        .collect();
    as_agent(
        h.service
            .commit(owner, commit(id, CommitBody::Ops(ops)), &trace()),
    )
    .await
}

fn landed(outcome: &CommitOutcome) -> bool {
    matches!(
        outcome,
        CommitOutcome::Committed(_) | CommitOutcome::Merged(_)
    )
}

#[tokio::test]
async fn rewriting_most_of_a_document_across_commits_is_a_broad_rewrite() {
    let h = agent_harness(
        AgentCap::Edit,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("broad-owner");
    let id = lines(&h, &owner, 8).await;
    let first = rewrite(&h, &owner, &id, &[0, 1, 2]).await.expect("narrow");
    assert!(landed(&first));
    let refused = rewrite(&h, &owner, &id, &[3, 4, 5])
        .await
        .expect_err("six of eight lines rewritten within the hour");
    match agent_refusal(&refused) {
        Some(AgentRefusal::BroadRewrite {
            rewritten, blocks, ..
        }) => {
            assert_eq!(*rewritten, 6);
            assert!(*blocks < 12, "{blocks}");
        },
        other => panic!("expected a broad rewrite refusal, got {other:?}"),
    }
    let (_, text) = h
        .service
        .plain_text(&owner, &id, None, &trace())
        .await
        .expect("text");
    assert!(text.contains("line 3"), "the refused commit stored nothing");
    assert!(text.contains("rewritten 0"));
    let person_edit = h
        .service
        .commit(
            &owner,
            commit(
                &id,
                CommitBody::Ops(vec![EditOp::StrReplace {
                    old_str: "line 7".to_owned(),
                    new_str: "owner 7".to_owned(),
                }]),
            ),
            &trace(),
        )
        .await
        .expect("a person is never weighed");
    assert!(landed(&person_edit));
}

#[tokio::test]
async fn a_policy_allowing_broad_rewrites_lets_the_agent_rewrite_everything() {
    let h = agent_harness(
        AgentCap::Edit,
        vec![AgentCapability::BroadRewrite],
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("broad-allowed");
    let id = lines(&h, &owner, 4).await;
    let outcome = rewrite(&h, &owner, &id, &[0, 1, 2, 3])
        .await
        .expect("allowed");
    assert!(landed(&outcome));
    let (_, text) = h
        .service
        .plain_text(&owner, &id, None, &trace())
        .await
        .expect("text");
    assert!(text.contains("rewritten 3"));
}

#[tokio::test]
async fn an_agent_restore_or_whole_file_body_needs_its_capability() {
    let h = agent_harness(
        AgentCap::Edit,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("restore-owner");
    let id = lines(&h, &owner, 2).await;
    let (head, _) = h
        .service
        .project(&owner, &id, None, &trace())
        .await
        .expect("project");
    let restore = commit(
        &id,
        CommitBody::Ops(vec![EditOp::Restore {
            revision_id: head.id,
        }]),
    );
    let refused = as_agent(h.service.commit(&owner, restore, &trace()))
        .await
        .expect_err("restore");
    assert!(matches!(
        agent_refusal(&refused),
        Some(AgentRefusal::HardRule {
            capability: AgentCapability::RestoreRevision,
            ..
        })
    ));
    let bytes = commit(&id, CommitBody::Bytes(b"whole file\n".to_vec()));
    let refused = as_agent(h.service.commit(&owner, bytes, &trace()))
        .await
        .expect_err("bytes");
    assert!(matches!(
        agent_refusal(&refused),
        Some(AgentRefusal::HardRule {
            capability: AgentCapability::ReplaceContent,
            ..
        })
    ));
    let (_, text) = h
        .service
        .plain_text(&owner, &id, None, &trace())
        .await
        .expect("text");
    assert_eq!(text, "line 0\nline 1\n");
}
