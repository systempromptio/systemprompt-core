use serde_json::json;
use systemprompt_brain::models::FactFilter;
use systemprompt_brain::repository::WorkingSetRepository;
use systemprompt_models::brain::FactScope;

use crate::events::entered;
use crate::harness::Harness;
use crate::personal_scope::mcp_search;
use crate::plane::{PRICING, kind};
use crate::recall_support::{ids, recall};

#[tokio::test]
async fn a_widened_row_left_by_an_older_binary_reaches_nobody_before_or_after_the_sweep() {
    let h = Harness::new("brain_stale_ws").await;
    let owner = h.user("owner", &[]).await;
    let other = h.user("other", &[]).await;
    h.emit(&entered(
        &owner,
        kind(PRICING),
        "acme",
        json!({"price": "1200", "region": "emea"}),
        FactScope::User(owner.clone()),
    ))
    .await;
    h.drain().await;
    let fact = h.only_fact("acme").await;
    h.service
        .confirm(&Harness::caller(&owner, &[]), &fact.id)
        .await
        .expect("confirm");
    sqlx::query(
        "INSERT INTO brain_working_set (scope_key, fact_id, importance_base) VALUES ('org', $1, 1.0)",
    )
    .bind(fact.id.as_str())
    .execute(&h.pg)
    .await
    .expect("stale row");
    assert_eq!(
        WorkingSetRepository::keys_for(&h.pg, &fact.id)
            .await
            .expect("keys"),
        vec!["org".to_owned(), format!("user:{owner}")]
    );

    let admitted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM v_brain_working_set WHERE fact_id = $1 AND scope_key = 'org'",
    )
    .bind(fact.id.as_str())
    .fetch_one(&h.pg)
    .await
    .expect("view");
    assert_eq!(admitted, 0, "the stale org row is not admitted");

    assert!(ids(&recall(&h, &other, "acme").await).is_empty());
    let listed = h
        .service
        .search(&Harness::caller(&other, &[]), &FactFilter::new())
        .await
        .expect("search");
    assert!(listed.facts.is_empty(), "{:?}", listed.facts);
    assert!(
        h.service
            .get(&Harness::caller(&other, &[]), &fact.id)
            .await
            .is_err()
    );
    assert_eq!(mcp_search(&h, &other).await, 0);
    assert_eq!(
        ids(&recall(&h, &owner, "acme").await),
        vec![fact.id.clone()],
        "negative control: the owner still recalls it"
    );

    h.pipeline
        .sweep(chrono::Utc::now())
        .await
        .expect("sweep")
        .expect("this replica swept");
    assert_eq!(
        WorkingSetRepository::keys_for(&h.pg, &fact.id)
            .await
            .expect("keys"),
        vec![format!("user:{owner}")],
        "the sweep removes the stale row"
    );
    h.finish().await;
}
