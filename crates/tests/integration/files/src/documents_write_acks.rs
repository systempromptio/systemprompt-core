//! Stored write acknowledgements and node id rows: an op id answers once
//! per document, acknowledgements past the replay window are swept, and a
//! revision's ids are written once.

use systemprompt_files::documents::models::{NodeIdEntry, StoredWriteAck};
use systemprompt_identifiers::{DocumentNodeId, DocumentOpId};
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_service_fixture::{GroupSource, allow, create, harness, person, trace};

#[tokio::test]
async fn an_op_id_keeps_its_first_ack_and_old_acks_are_swept() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("acks");
    let (document, revision) = h
        .service
        .create_from_template(&owner, create("blank-markdown", None), &trace())
        .await
        .expect("create");
    let op_id = DocumentOpId::new(format!("op-{}", uuid::Uuid::new_v4()));
    let first = StoredWriteAck {
        revision_id: revision.id.clone(),
        ack: serde_json::json!({ "revision_id": revision.id.as_str(), "applied": 1 }),
    };
    h.service
        .record_write_ack(&document.id, &op_id, &first)
        .await
        .expect("record");
    let second = StoredWriteAck {
        ack: serde_json::json!({ "applied": 2 }),
        ..first.clone()
    };
    h.service
        .record_write_ack(&document.id, &op_id, &second)
        .await
        .expect("record again");
    let found = h
        .service
        .find_write_ack(&owner, &document.id, &op_id, &trace())
        .await
        .expect("find")
        .expect("stored ack");
    assert_eq!(found, first);

    let pool = test_db_pool().await.pool();
    sqlx::query(
        "UPDATE document_write_acks SET created_at = now() - interval '8 days' \
         WHERE document_id = $1::uuid AND op_id = $2",
    )
    .bind(document.id.as_str())
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
            .find(&document.id, &op_id)
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
        .expect("project mints ids");
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
    h.repos
        .node_ids
        .insert(&document.id, &revision.id, &rival)
        .await
        .expect("second insert is a no-op");
    assert_eq!(
        h.repos.node_ids.list(&revision.id).await.expect("relist"),
        stored
    );
    let writer = stored[0]
        .node_id
        .as_str()
        .split_once('.')
        .expect("writer.counter")
        .0
        .to_owned();
    let last = h
        .repos
        .node_ids
        .max_counter(&document.id, &writer)
        .await
        .expect("max counter");
    assert_eq!(last, i64::try_from(stored.len()).expect("count"));
}
