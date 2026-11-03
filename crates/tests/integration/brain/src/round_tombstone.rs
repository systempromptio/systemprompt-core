//! A tombstone written after a consumer round preloaded its cache is still
//! honoured: the cached snapshot covers the identity and holds no
//! tombstone, yet the candidate is re-checked against the store at write
//! time and refused, so an identity erased while the round ran is never
//! re-inserted past its tombstone.

use systemprompt_brain::index::Reindex;
use systemprompt_brain::pipeline::round::RoundCache;
use systemprompt_brain::pipeline::{BatchContext, ProcessOutcome, ProcessParams, rules};
use systemprompt_brain::repository::TombstoneRepository;
use systemprompt_models::brain::{FactScope, SubjectKey};

use crate::events::{PRODUCT, entered, price};
use crate::harness::{Harness, Setup};
use crate::plane::{PRICING, kind};

#[tokio::test]
async fn a_tombstone_written_after_the_preload_is_honoured() {
    let h = Harness::with("brain_round_tombstone", Setup::confirming()).await;
    let author = h.user("author", &[]).await;
    let fields = price("acme", "emea", "1200");
    let event = entered(
        &author,
        kind(PRICING),
        "acme",
        fields.clone(),
        FactScope::Org,
    );
    let id = h.emit(&event).await;

    let mut tx = h.pg.begin().await.expect("begin");
    let ctx = BatchContext::load(&mut tx).await.expect("batch context");
    let cache = RoundCache::load(&mut tx, &ctx, &[(id.clone(), event.clone())])
        .await
        .expect("preload");
    assert!(cache.claimed(&id), "the round claimed the event");
    let declared = ctx.kind(&kind(PRICING)).expect("pricing kind");
    let serde_json::Value::Object(map) = fields else {
        panic!("fields are an object");
    };
    let identity = rules::identity_of(declared, &SubjectKey::new(PRODUCT, "acme"), &map);
    let covered = cache
        .snapshot(&kind(PRICING), &SubjectKey::new(PRODUCT, "acme"), &identity)
        .expect("the cache covers the entered identity");
    assert!(!covered.tombstoned, "nothing was erased before the preload");

    let mut erased = h.pg.acquire().await.expect("a console connection");
    TombstoneRepository::insert(&mut erased, &kind(PRICING), &identity, None)
        .await
        .expect("a tombstone lands while the round runs");
    drop(erased);

    let params = ProcessParams {
        ctx: &ctx,
        reindex: Reindex::Deferred,
        round: Some(&cache),
    };
    let outcome = h
        .pipeline
        .process_with(&id, event, &mut tx, &params)
        .await
        .expect("process");
    tx.commit().await.expect("commit");

    let ProcessOutcome::Finished(summary) = outcome else {
        panic!("the claimed event is processed, not skipped");
    };
    assert_eq!(summary.tally.written, 0, "{summary:?}");
    assert_eq!(
        summary.tally.rejected, 1,
        "the tombstone refused the insert"
    );
    assert!(summary.facts.is_empty(), "{summary:?}");
    assert!(
        h.fact_ids("acme").await.is_empty(),
        "no fact was inserted past the tombstone"
    );
    h.finish().await;
}
