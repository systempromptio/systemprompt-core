use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_provider_contracts::documents::content_hash;
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_media_read::{png, store, with_image};
use crate::documents_service_fixture::{GroupSource, Harness, allow, harness, person, trace};

const MIGRATION_SQL: &str =
    include_str!("../../../../domain/files/schema/migrations/039_document_media_backfill.sql");

async fn exec(sql: &'static str, document: &DocumentId) {
    let pool = test_db_pool().await.write_pool();
    sqlx::query(sql)
        .bind(document.as_str())
        .execute(&*pool)
        .await
        .expect("exec");
}

async fn count(sql: &'static str, key: &str) -> i64 {
    let pool = test_db_pool().await.write_pool();
    sqlx::query_scalar(sql)
        .bind(key)
        .fetch_one(&*pool)
        .await
        .expect("count")
}

async fn as_written_before_admissions(document: &DocumentId) {
    exec(
        "DELETE FROM document_media WHERE document_id = $1::uuid",
        document,
    )
    .await;
    exec(
        "INSERT INTO document_media_backfill (document_id) VALUES ($1::uuid)
         ON CONFLICT (document_id) DO NOTHING",
        document,
    )
    .await;
}

async fn queued(document: &DocumentId) -> bool {
    count(
        "SELECT COUNT(*) FROM document_media_backfill WHERE document_id = $1::uuid",
        document.as_str(),
    )
    .await
        == 1
}

async fn blob_stored(hash: &str) -> bool {
    count(
        "SELECT COUNT(*) FROM document_blobs WHERE content_sha256 = $1",
        hash,
    )
    .await
        == 1
}

async fn age(hash: &str) {
    let pool = test_db_pool().await.write_pool();
    sqlx::query(
        "UPDATE document_blobs SET created_at = now() - interval '3650 days'
         WHERE content_sha256 = $1",
    )
    .bind(hash)
    .execute(&*pool)
    .await
    .expect("age");
}

async fn pre_admission_markdown(h: &Harness, owner: &AuthenticatedUser) -> (DocumentId, String) {
    let side = uuid::Uuid::new_v4().as_u128();
    let picture = png((side >> 64) as u32 | 1, side as u32 | 1);
    store(h).put(&picture).await.expect("put blob");
    let document = with_image(h, owner, "blank-markdown", &picture).await;
    as_written_before_admissions(&document).await;
    (document, content_hash(&picture).as_str().to_owned())
}

#[tokio::test]
async fn the_migration_queues_every_document_that_links_images() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("backfill-migration");
    let picture = png(31, 17);
    store(&h).put(&picture).await.expect("put blob");
    let document = with_image(&h, &owner, "blank-markdown", &picture).await;

    let pool = test_db_pool().await.write_pool();
    let mut tx = pool.begin().await.expect("begin");
    sqlx::raw_sql(MIGRATION_SQL)
        .execute(&mut *tx)
        .await
        .expect("migration");
    let queued: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM document_media_backfill WHERE document_id = $1::uuid",
    )
    .bind(document.as_str())
    .fetch_one(&mut *tx)
    .await
    .expect("probe");
    tx.rollback().await.expect("rollback");

    assert_eq!(queued, 1);
}

#[tokio::test]
async fn an_image_written_before_admissions_survives_gc_and_keeps_serving() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("backfill-gc");
    let (document, hash) = pre_admission_markdown(&h, &owner).await;
    age(&hash).await;

    store(&h).delete_unreferenced(24, 500).await.expect("gc");
    assert!(
        blob_stored(&hash).await,
        "a queued document's image is held"
    );

    h.service
        .backfill_queued_media(1000)
        .await
        .expect("backfill");
    assert!(!queued(&document).await);
    store(&h).delete_unreferenced(24, 500).await.expect("gc");

    assert!(blob_stored(&hash).await, "the admitted image is kept");
    let served = h
        .service
        .held_image(&owner, &document, &hash, &trace())
        .await
        .expect("served");
    assert_eq!(served.mime, "image/png");
}

#[tokio::test]
async fn a_queued_document_admits_its_images_on_first_read() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("backfill-read");
    let (document, hash) = pre_admission_markdown(&h, &owner).await;

    let served = h
        .service
        .held_image(&owner, &document, &hash, &trace())
        .await
        .expect("served");

    assert_eq!(served.mime, "image/png");
    assert!(!queued(&document).await);
    assert_eq!(
        count(
            "SELECT COUNT(*) FROM document_media WHERE document_id = $1::uuid",
            document.as_str(),
        )
        .await,
        1
    );
    assert_eq!(
        count(
            "SELECT COUNT(*) FROM document_media WHERE document_id = $1::uuid
               AND admitted_by = (SELECT author_user_id FROM document_revisions
                                   WHERE document_id = $1::uuid
                                   ORDER BY created_at LIMIT 1)",
            document.as_str(),
        )
        .await,
        1,
        "admitted by the author of the first revision naming it"
    );
}
