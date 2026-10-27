//! Stored write acknowledgements and node id rows: an acknowledgement past
//! the replay window is swept, and a revision's ids are written once.

use systemprompt_files::documents::models::NodeIdEntry;
use systemprompt_files::documents::repository::DocumentNodeIdRepository;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommentPolicy, CommitRequest};
use systemprompt_identifiers::{DocumentNodeId, DocumentOpId};
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_proposals::{memo, outline, rewrite};
use crate::documents_service_fixture::{
    GroupSource, allow, api_author, create, harness, person, trace, unique,
};

#[tokio::test]
async fn acks_past_the_replay_window_are_swept() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("acks");
    let (document, _) = memo(&h, &owner, None).await;
    let head = outline(&h, &owner, &document, None).await;
    let op_id = DocumentOpId::new(unique("op"));
    h.service
        .commit(
            &owner,
            CommitRequest {
                document_id: document.clone(),
                base_revision: None,
                body: CommitBody::Ops(vec![rewrite(&head, "beta", "BETA")]),
                author: api_author(),
                message: None,
                lock_holder_id: None,
                comments: CommentPolicy::default(),
                op_id: Some(op_id.clone()),
            },
            &trace(),
        )
        .await
        .expect("commit");
    assert!(
        h.repos
            .write_acks
            .find(&document, &op_id)
            .await
            .expect("find")
            .is_some()
    );

    let pool = test_db_pool().await.pool();
    sqlx::query(
        "UPDATE document_write_acks SET created_at = now() - interval '8 days' \
         WHERE document_id = $1::uuid AND op_id = $2",
    )
    .bind(document.as_str())
    .bind(op_id.as_str())
    .execute(&*pool)
    .await
    .expect("age the ack");
    let swept = h
        .repos
        .write_acks
        .sweep_older_than(7, 10_000)
        .await
        .expect("sweep");
    assert!(swept >= 1);
    assert!(
        h.repos
            .write_acks
            .find(&document, &op_id)
            .await
            .expect("find after sweep")
            .is_none()
    );
}

#[tokio::test]
async fn a_revisions_ids_are_written_once() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("node-ids");
    let (document, revision) = h
        .service
        .create_from_template(&owner, create("blank-markdown", None), &trace())
        .await
        .expect("create");
    let (_, projected) = h
        .service
        .project(&owner, &document.id, Some(&revision.id), &trace())
        .await
        .expect("project");
    let stored = h.repos.node_ids.list(&revision.id).await.expect("list");
    assert_eq!(stored.len(), projected.nodes().count());
    let rival: Vec<NodeIdEntry> = stored
        .iter()
        .enumerate()
        .map(|(n, entry)| NodeIdEntry {
            node_id: DocumentNodeId::new(format!("rival.{}", n + 1)),
            ..entry.clone()
        })
        .collect();
    let mut tx = h.repos.documents.begin().await.expect("begin");
    let second =
        DocumentNodeIdRepository::insert(&mut tx, &document.id, &revision.id, &rival).await;
    assert!(
        second.is_err(),
        "a second write of a revision's ids is refused"
    );
    drop(tx);
    assert_eq!(
        h.repos.node_ids.list(&revision.id).await.expect("relist"),
        stored
    );
}
