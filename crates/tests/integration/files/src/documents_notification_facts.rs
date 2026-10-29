//! Every document write that notifies records exactly one notification fact
//! carrying all of its direct recipients: a comment mentioning several users
//! and a share to a role with several members each append a single outbox
//! row, never one row per recipient.

use std::sync::Arc;

use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::models::{CommentAnchor, GrantTarget};
use systemprompt_files::documents::service::{CommentRequest, ShareRequest};
use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{AccessLevel, EditOp};
use systemprompt_models::notifications::{NOTIFICATIONS_CONSUMER, NotificationKind};
use systemprompt_test_fixtures::test_db_pool;
use tempfile::TempDir;

use crate::documents_service_fixture::{
    GroupSource, Harness, RoleDirectory, allow, create, harness_configured, person, trace, unique,
};

async fn text_document(h: &Harness, owner: &AuthenticatedUser) -> DocumentId {
    let mut request = create("blank-text", None);
    request.ops = vec![EditOp::SetText {
        text: "alpha beta\n".to_owned(),
    }];
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

async fn facts_naming(marker: &str, kind: &str) -> Vec<i32> {
    let db = test_db_pool().await;
    sqlx::query_scalar(
        "SELECT jsonb_array_length(fact->'data'->'direct_recipients') FROM event_outbox \
         WHERE consumer = $1 AND channel = 'fact' \
         AND fact->'data'->>'kind' = $2 AND fact::text LIKE '%' || $3 || '%'",
    )
    .bind(NOTIFICATIONS_CONSUMER)
    .bind(kind)
    .bind(marker)
    .fetch_all(db.write_pool().as_ref())
    .await
    .expect("outbox facts")
}

#[tokio::test]
async fn a_comment_mentioning_several_users_appends_one_fact() {
    let h = harness_configured(
        allow(),
        GroupSource::new(&[]),
        Arc::new(TempDir::new().expect("tempdir")),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("facts-mention-owner");
    let mentioned: Vec<UserId> = (0..3)
        .map(|n| person(&format!("facts-mentioned-{n}")).id)
        .collect();
    let document = text_document(&h, &owner).await;
    let (_, projection) = h
        .service
        .project(&owner, &document, None, &trace())
        .await
        .expect("project");
    let node = projection
        .nodes()
        .find(|node| node.text.as_deref() == Some("alpha beta"))
        .expect("line");
    let mut body = "look here".to_owned();
    for user in &mentioned {
        body.push_str(&format!(" @[Someone](user:{user})"));
    }
    let comment = h
        .service
        .create_comment(
            &owner,
            CommentRequest {
                document: document.clone(),
                anchor: Some(CommentAnchor {
                    node: node.id.clone(),
                    find: None,
                    nth: None,
                }),
                range: None,
                body,
                mentions: mentioned.clone(),
                parent_id: None,
                mention_options: vec![],
            },
            &trace(),
        )
        .await
        .expect("comment");

    assert_eq!(
        facts_naming(comment.id.as_str(), NotificationKind::MENTIONED).await,
        vec![3],
        "one mention fact carries all three mentioned users"
    );
}

#[tokio::test]
async fn a_share_to_a_role_appends_one_fact_for_all_members() {
    let members: Vec<AuthenticatedUser> = (0..4)
        .map(|n| person(&format!("facts-role-member-{n}")))
        .collect();
    let role = unique("facts-role");
    let refs: Vec<&UserId> = members.iter().map(|member| &member.id).collect();
    let h = harness_configured(
        allow(),
        GroupSource::new(&[]),
        Arc::new(TempDir::new().expect("tempdir")),
        RoleDirectory::default().with_role(&role, &refs),
        DocumentsConfig::default(),
    )
    .await;
    let owner = person("facts-share-owner");
    let document = text_document(&h, &owner).await;
    h.service
        .share(
            &owner,
            ShareRequest {
                target: GrantTarget::Document(document.clone()),
                subject_kind: "role".to_owned(),
                subject_value: role.clone(),
                subject_display: None,
                level: AccessLevel::View,
                valid_until: None,
                message: None,
                via_agent: None,
            },
            &trace(),
        )
        .await
        .expect("share");

    assert_eq!(
        facts_naming(&role, NotificationKind::SHARED).await,
        vec![4],
        "one share fact carries every member of the role"
    );
}
