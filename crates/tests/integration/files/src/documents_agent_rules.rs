//! The rules an agent meets whatever its cap: no sharing, no grant through
//! a mention, no trash unless the policy allows it; an audited read under
//! a cap below edit; and the hourly ceiling on agent edits.

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::agents::{AgentCap, AgentCapability, AgentRefusal};
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::models::GrantTarget;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommentRequest, CommitRequest, ShareRequest,
};
use systemprompt_files::{DocumentLimits, DocumentsConfig};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::documents::{AccessLevel, EditOp};
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_agent_fixture::{agent_harness, agent_refusal, as_agent, document, edit};
use crate::documents_service_fixture::{RoleDirectory, api_author, person, trace};

fn share_with(id: &DocumentId, user: &str) -> ShareRequest {
    ShareRequest {
        target: GrantTarget::Document(id.clone()),
        subject_kind: "user".to_owned(),
        subject_value: user.to_owned(),
        subject_display: None,
        level: AccessLevel::View,
        valid_until: None,
        message: None,
        via_agent: Some("claude-code".to_owned()),
    }
}

fn hard_rule(error: &DocumentsError) -> Option<AgentCapability> {
    match agent_refusal(error) {
        Some(AgentRefusal::HardRule { capability, .. }) => Some(*capability),
        _ => None,
    }
}

#[tokio::test]
async fn an_agent_cannot_share_mention_grant_or_trash_under_an_edit_cap() {
    let owner = person("rules-owner");
    let colleague = person("rules-colleague");
    let directory = RoleDirectory::default()
        .with_user(&owner.id, "Owner")
        .with_user(&colleague.id, "Colleague");
    let h = agent_harness(
        AgentCap::Edit,
        Vec::new(),
        directory,
        DocumentsConfig::default(),
    )
    .await;
    let id = document(&h, &owner).await;

    let share = as_agent(
        h.service
            .share(&owner, share_with(&id, colleague.id.as_str()), &trace()),
    )
    .await
    .expect_err("agents do not share");
    assert_eq!(hard_rule(&share), Some(AgentCapability::Share));

    let mention = CommentRequest {
        document: id.clone(),
        anchor: None,
        range: None,
        body: format!("@[Colleague](user:{}) please look", colleague.id),
        mentions: vec![colleague.id.clone()],
        parent_id: None,
        mention_options: vec![],
    };
    let mentioned = as_agent(h.service.create_comment(&owner, mention, &trace()))
        .await
        .expect_err("an agent's mention never grants");
    assert_eq!(hard_rule(&mentioned), Some(AgentCapability::MentionGrant));
    let levels = h
        .service
        .levels_on(&owner, &id, std::slice::from_ref(&colleague.id), &trace())
        .await
        .expect("levels");
    assert_eq!(levels, vec![None]);

    let trashed = as_agent(h.service.trash(&owner, &id, &trace()))
        .await
        .expect_err("agents do not trash");
    assert_eq!(hard_rule(&trashed), Some(AgentCapability::Trash));

    h.service
        .share(&owner, share_with(&id, colleague.id.as_str()), &trace())
        .await
        .expect("the owner, directly, shares");
}

#[tokio::test]
async fn a_policy_that_allows_sharing_lets_the_agent_share() {
    let owner = person("allow-owner");
    let colleague = person("allow-colleague");
    let directory = RoleDirectory::default().with_user(&colleague.id, "Colleague");
    let h = agent_harness(
        AgentCap::Edit,
        vec![AgentCapability::Share],
        directory,
        DocumentsConfig::default(),
    )
    .await;
    let id = document(&h, &owner).await;
    as_agent(
        h.service
            .share(&owner, share_with(&id, colleague.id.as_str()), &trace()),
    )
    .await
    .expect("the policy allows it");
}

#[tokio::test]
async fn an_agent_read_under_a_cap_below_edit_is_audited_with_its_trace() {
    let h = agent_harness(
        AgentCap::Read,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("audit-owner");
    let id = document(&h, &owner).await;
    let traced = trace();
    as_agent(h.service.get(&owner, &id, &traced))
        .await
        .expect("read");
    let pool = test_db_pool().await.pool();
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT agent_name, effective_cap, trace_id FROM document_agent_reads
         WHERE document_id = $1::text::uuid",
    )
    .bind(id.as_str())
    .fetch_all(&*pool)
    .await
    .expect("audit rows");
    assert_eq!(
        rows,
        vec![(
            "claude-code".to_owned(),
            "read".to_owned(),
            traced.as_str().to_owned()
        )]
    );
}

#[tokio::test]
async fn agent_edits_past_the_hourly_ceiling_are_refused() {
    let config = DocumentsConfig {
        limits: DocumentLimits {
            agent_edits_per_hour: 2,
            ..DocumentLimits::default()
        },
        ..DocumentsConfig::default()
    };
    let h = agent_harness(AgentCap::Edit, Vec::new(), RoleDirectory::default(), config).await;
    let owner = person("rate-owner");
    let id = document(&h, &owner).await;
    as_agent(edit(&h, &owner, &id, "one\n"))
        .await
        .expect("first");
    as_agent(edit(&h, &owner, &id, "two\n"))
        .await
        .expect("second");
    let refused = as_agent(edit(&h, &owner, &id, "three\n"))
        .await
        .expect_err("third");
    assert!(matches!(
        agent_refusal(&refused),
        Some(AgentRefusal::RateLimited { limit: 2, .. })
    ));
    edit(&h, &owner, &id, "the owner is not limited\n")
        .await
        .expect("a person is not limited");
}

#[tokio::test]
async fn an_agents_set_text_is_a_whole_content_replace() {
    let h = agent_harness(
        AgentCap::Edit,
        Vec::new(),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("replace-owner");
    let id = document(&h, &owner).await;
    let head = h
        .service
        .get(&owner, &id, &trace())
        .await
        .expect("get")
        .head
        .map(|r| r.id);
    let request = CommitRequest {
        document_id: id.clone(),
        base_revision: head,
        body: CommitBody::Ops(vec![EditOp::SetText {
            text: "all of it\n".to_owned(),
        }]),
        author: api_author(),
        message: None,
        lock_holder_id: None,
        comments: CommentPolicy::default(),
        op_id: None,
        media: Media::new(),
    };
    let refused = as_agent(h.service.commit(&owner, request, &trace()))
        .await
        .expect_err("set_text is a whole-content replace");
    assert_eq!(hard_rule(&refused), Some(AgentCapability::ReplaceContent));
}
