//! Two identical recalls over a few thousand candidate facts answer the
//! identical block: every tier orders by rank, stored importance and id,
//! and the fill breaks ties by id, so nothing depends on scan order.

use systemprompt_brain::repository::{SearchIndexRepository, WorkingSetRepository};
use systemprompt_extension::brain::MemoryRecall;
use systemprompt_models::brain::SynonymsDoc;

use crate::harness::{Harness, Setup};
use crate::recall_seed::{HINTS, PROJECT, seed};
use crate::recall_support::{injected, principal, request};

const BULK: i64 = 3000;

async fn seed_bulk(h: &Harness, template: &str) {
    sqlx::query(
        "INSERT INTO brain_facts
             (id, kind, subject_kind, subject_id, scope_level, scope_key, statement, fields,
              source, source_kind, status, importance_base, identity_hash, statement_hash,
              owner_user_id)
         SELECT 'm_' || gen_random_uuid()::text, t.kind, t.subject_kind, 'acme', 'org', NULL,
                'bulk ' || n || ' acme payments emea', '{}'::jsonb, t.source, t.source_kind,
                'confirmed', ((n % 7)::float8) / 10, 'bulk-identity-' || n, 'bulk-statement-' || n,
                t.owner_user_id
         FROM generate_series(1, $2) AS n, brain_facts t
         WHERE t.id = $1",
    )
    .bind(template)
    .bind(BULK)
    .execute(&h.pg)
    .await
    .expect("bulk insert");
    let mut conn = h.pg.acquire().await.expect("connection");
    WorkingSetRepository::rebuild_all(&mut conn)
        .await
        .expect("working set");
    SearchIndexRepository::rebuild_all(&mut conn, &SynonymsDoc::default())
        .await
        .expect("search index");
}

#[tokio::test]
async fn two_identical_recalls_over_three_thousand_facts_produce_identical_blocks() {
    let h = Harness::with("brain_recall_determinism", Setup::confirming()).await;
    let seeded = seed(&h).await;
    seed_bulk(&h, seeded.subject.as_str()).await;
    assert!(h.total("brain_facts").await > BULK);

    let service = h.recall();
    let req = request(principal(&seeded.reader, Some(PROJECT)), HINTS);
    let first = service.recall(&req).await.expect("first");
    let second = service.recall(&req).await.expect("second");
    let (a, b) = (
        injected(&first).expect("first block"),
        injected(&second).expect("second block"),
    );
    assert_eq!(a.ids(), b.ids());
    assert_eq!(a.withheld, b.withheld);
    assert_eq!(a.tokens, b.tokens);
    assert!(!a.facts.is_empty());
    let mut lines_a = a.render();
    let mut lines_b = b.render();
    lines_a.replace_range(..lines_a.find('\n').expect("header"), "");
    lines_b.replace_range(..lines_b.find('\n').expect("header"), "");
    assert_eq!(lines_a, lines_b, "identical bodies; only as_of differs");
    h.finish().await;
}
