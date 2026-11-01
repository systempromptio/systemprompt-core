use chrono::Utc;
use systemprompt_brain::BrainError;
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::auth::RequestActor;
use systemprompt_models::brain::{
    BrainFact, FactStatus, RecallCtx, RevisionChange, WithheldReason,
};

use crate::events::{Landed, price};
use crate::harness::Harness;

async fn captured(h: &Harness, product: &str) -> (UserId, BrainFact) {
    let owner = h.user("owner", &[]).await;
    h.model
        .push(serde_json::json!([price(product, "emea", "1200")]));
    h.emit(&Landed::new().event(&owner)).await;
    h.drain().await;
    let fact = h.only_fact(product).await;
    (owner, fact)
}

fn fields(price: &str, region: &str) -> serde_json::Map<String, serde_json::Value> {
    let serde_json::Value::Object(map) = serde_json::json!({ "price": price, "region": region })
    else {
        unreachable!("object literal")
    };
    map
}

#[tokio::test]
async fn an_unknown_fact_reads_as_not_found() {
    let h = Harness::new("brain_reads_missing").await;
    let user = h.user("reader", &[]).await;
    let missing = BrainFactId::generate();
    let error = h
        .service
        .get(&Harness::caller(&user, &[]), &missing)
        .await
        .expect_err("no such fact");
    assert!(
        matches!(error, BrainError::NotFound(ref id) if *id == missing),
        "{error:?}"
    );
    let error = h
        .service
        .timeline(&Harness::caller(&user, &[]), &missing)
        .await
        .expect_err("no timeline for a missing fact");
    assert!(matches!(error, BrainError::NotFound(_)), "{error:?}");
    h.finish().await;
}

#[tokio::test]
async fn the_timeline_lists_every_revision_in_order_() {
    let h = Harness::new("brain_reads_timeline").await;
    let (owner, fact) = captured(&h, "acme").await;
    let caller = Harness::caller(&owner, &[]);
    h.service.confirm(&caller, &fact.id).await.expect("confirm");
    h.service.pin(&caller, &fact.id).await.expect("pin");
    let unpinned = h.service.unpin(&caller, &fact.id).await.expect("unpin");
    assert_eq!(unpinned.change, RevisionChange::Unpinned);
    assert!(!h.fact(&fact.id).await.pinned);

    let timeline = h
        .service
        .timeline(&caller, &fact.id)
        .await
        .expect("timeline");
    let changes: Vec<RevisionChange> = timeline.iter().map(|r| r.change).collect();
    assert_eq!(
        changes,
        vec![
            RevisionChange::Created,
            RevisionChange::Confirmed,
            RevisionChange::Pinned,
            RevisionChange::Unpinned,
        ]
    );
    let numbers: Vec<i32> = timeline.iter().map(|r| r.revision).collect();
    assert!(numbers.windows(2).all(|w| w[0] < w[1]), "{numbers:?}");

    h.finish().await;
}

#[tokio::test]
async fn why_reports_the_terms_and_withholds_a_proposed_fact_only_once_retracted() {
    let h = Harness::new("brain_reads_why").await;
    let (owner, fact) = captured(&h, "acme").await;
    let caller = Harness::caller(&owner, &[]);
    h.service.confirm(&caller, &fact.id).await.expect("confirm");
    let ctx = RecallCtx::new(owner.clone(), RequestActor::Human, Utc::now());

    let why = h.service.why(&caller, &fact.id, &ctx).await.expect("why");
    assert_eq!(why.fact.id, fact.id);
    assert_eq!(why.withheld, None);
    let terms = why.terms;
    assert!(terms.total.is_finite() && terms.total > 0.0, "{terms:?}");
    assert!(
        (terms.subject).abs() < f64::EPSILON,
        "no subject match: {terms:?}"
    );

    h.service
        .retract(&caller, &fact.id, "gone")
        .await
        .expect("retract");
    let why = h.service.why(&caller, &fact.id, &ctx).await.expect("why");
    assert_eq!(why.fact.status, FactStatus::Retired);
    assert_eq!(why.withheld, Some(WithheldReason::Scope));
    h.finish().await;
}

#[tokio::test]
async fn why_names_review_due_once_the_review_clock_lapses() {
    let h = Harness::new("brain_reads_why_due").await;
    let (owner, fact) = captured(&h, "acme").await;
    let caller = Harness::caller(&owner, &[]);
    h.service.confirm(&caller, &fact.id).await.expect("confirm");
    sqlx::query("UPDATE brain_facts SET review_after = now() - interval '1 day' WHERE id = $1")
        .bind(fact.id.as_str())
        .execute(&h.pg)
        .await
        .expect("age");
    let ctx = RecallCtx::new(owner.clone(), RequestActor::Human, Utc::now());
    let why = h.service.why(&caller, &fact.id, &ctx).await.expect("why");
    assert_eq!(why.withheld, Some(WithheldReason::ReviewDue));
    h.finish().await;
}

#[tokio::test]
async fn a_field_edit_rerenders_the_statement_and_records_a_revision() {
    let h = Harness::new("brain_reads_edit").await;
    let (owner, fact) = captured(&h, "acme").await;
    let caller = Harness::caller(&owner, &[]);

    let revision = h
        .service
        .edit_fields(&caller, &fact.id, fields("1500", "emea"))
        .await
        .expect("edit");
    assert_eq!(revision.change, RevisionChange::FieldsEdited);

    let edited = h.fact(&fact.id).await;
    assert_eq!(edited.fields.get("price"), Some(&serde_json::json!("1500")));
    assert!(edited.statement.contains("1500"), "{}", edited.statement);
    assert!(!edited.statement.contains("1200"), "{}", edited.statement);
    assert_eq!(edited.identity_hash, fact.identity_hash);
    h.finish().await;
}

#[tokio::test]
async fn an_edit_that_changes_identity_or_comes_from_a_stranger_is_refused() {
    let h = Harness::new("brain_reads_edit_refused").await;
    let (owner, fact) = captured(&h, "acme").await;

    let error = h
        .service
        .edit_fields(
            &Harness::caller(&owner, &[]),
            &fact.id,
            fields("1500", "apac"),
        )
        .await
        .expect_err("region is an identity field");
    assert!(matches!(error, BrainError::Forbidden { .. }), "{error:?}");

    let stranger = h.user("stranger", &[]).await;
    let error = h
        .service
        .edit_fields(
            &Harness::caller(&stranger, &[]),
            &fact.id,
            fields("1", "emea"),
        )
        .await
        .expect_err("a stranger may not edit");
    assert!(
        matches!(
            error,
            BrainError::Forbidden { .. } | BrainError::NotFound(_)
        ),
        "{error:?}"
    );

    let unchanged = h.fact(&fact.id).await;
    assert_eq!(unchanged.statement, fact.statement);
    assert_eq!(unchanged.fields, fact.fields);
    h.finish().await;
}
