//! An author mentioning themselves is delivered only at `agent` attention,
//! the hand-off to their own agent: the mention fact carries them as a direct
//! recipient at `agent` and they receive the live
//! `document.comment.mentioned`. At `human` attention, or with none chosen,
//! the self-mention records no fact and sends no mention event.

use std::sync::Arc;
use std::time::Duration;

use systemprompt_events::Broadcaster;
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::models::CommentAnchor;
use systemprompt_files::documents::service::{CommentRequest, MentionOption};
use systemprompt_identifiers::{ConnectionId, DocumentCommentId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::EditOp;
use systemprompt_models::notifications::{Attention, NOTIFICATIONS_CONSUMER, NotificationKind};
use systemprompt_test_fixtures::test_db_pool;
use tempfile::TempDir;
use tokio::sync::mpsc;

use crate::documents_service_fixture::{
    GroupSource, Harness, RoleDirectory, allow, create, harness_configured, person, trace,
};

async fn configured() -> Harness {
    harness_configured(
        allow(),
        GroupSource::new(&[]),
        Arc::new(TempDir::new().expect("tempdir")),
        RoleDirectory::default(),
        DocumentsConfig::default(),
    )
    .await
}

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

async fn self_mention(
    h: &Harness,
    author: &AuthenticatedUser,
    attention: Option<Attention>,
) -> DocumentCommentId {
    let document = text_document(h, author).await;
    let (_, projection) = h
        .service
        .project(author, &document, None, &trace())
        .await
        .expect("project");
    let node = projection
        .nodes()
        .find(|node| node.text.as_deref() == Some("alpha beta"))
        .expect("line")
        .id
        .clone();
    let mention_options = attention
        .map(|attention| MentionOption {
            user_id: author.id.clone(),
            attention,
        })
        .into_iter()
        .collect();
    h.service
        .create_comment(
            author,
            CommentRequest {
                document,
                anchor: Some(CommentAnchor {
                    node,
                    find: None,
                    nth: None,
                    cells: None,
                }),
                range: None,
                body: format!("pick this up @[Me](user:{})", author.id),
                mentions: vec![author.id.clone()],
                parent_id: None,
                mention_options,
            },
            &trace(),
        )
        .await
        .expect("comment")
        .id
}

async fn mention_recipients(comment: &DocumentCommentId) -> Vec<serde_json::Value> {
    let db = test_db_pool().await;
    sqlx::query_scalar(
        "SELECT fact->'data'->'direct_recipients' FROM event_outbox \
         WHERE consumer = $1 AND channel = 'fact' \
         AND fact->'data'->>'kind' = $2 AND fact::text LIKE '%' || $3 || '%'",
    )
    .bind(NOTIFICATIONS_CONSUMER)
    .bind(NotificationKind::MENTIONED)
    .bind(comment.as_str())
    .fetch_all(db.write_pool().as_ref())
    .await
    .expect("outbox facts")
}

async fn seen<T: std::fmt::Debug>(rx: &mut mpsc::Receiver<T>) -> String {
    let mut seen = String::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
        seen.push_str(&format!("{event:?}"));
    }
    seen
}

#[tokio::test]
async fn a_self_mention_at_agent_attention_is_delivered() {
    let h = configured().await;
    let author = person("self-mention-agent");
    let (tx, mut rx) = mpsc::channel(32);
    assert!(
        h.broadcasters
            .agui
            .register(&author.id, &ConnectionId::generate(), tx)
            .await
    );
    let comment = self_mention(&h, &author, Some(Attention::Agent)).await;

    let facts = mention_recipients(&comment).await;
    assert_eq!(facts.len(), 1, "one mention fact");
    assert_eq!(
        facts[0],
        serde_json::json!([{ "user": author.id, "attention": "agent" }]),
        "the author is the direct recipient at agent attention"
    );
    assert!(
        seen(&mut rx).await.contains("document.comment.mentioned"),
        "the author receives the live mention"
    );
}

#[tokio::test]
async fn a_self_mention_at_human_or_no_attention_stays_suppressed() {
    let h = configured().await;
    for attention in [Some(Attention::Human), Some(Attention::Silent), None] {
        let author = person("self-mention-quiet");
        let (tx, mut rx) = mpsc::channel(32);
        assert!(
            h.broadcasters
                .agui
                .register(&author.id, &ConnectionId::generate(), tx)
                .await
        );
        let comment = self_mention(&h, &author, attention).await;
        assert!(
            mention_recipients(&comment).await.is_empty(),
            "{attention:?}: no mention fact"
        );
        let seen = seen(&mut rx).await;
        assert!(
            !seen.contains("document.comment.mentioned"),
            "{attention:?}: no live mention: {seen}"
        );
    }
}
