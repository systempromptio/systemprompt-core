use systemprompt_brain::repository::WorkingSetRepository;
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::brain::FactScope;

use crate::events::entered;
use crate::harness::Harness;
use crate::plane::{OWNERSHIP, PRICING, kind};

pub(crate) const PROJECT: &str = "proj-x";
pub(crate) const HINTS: &str = "acme payments stalex retiredx disputedx duex";

pub(crate) struct Seeded {
    pub(crate) reader: UserId,
    pub(crate) pinned: BrainFactId,
    pub(crate) subject: BrainFactId,
    pub(crate) context: BrainFactId,
    pub(crate) full_text: BrainFactId,
    pub(crate) excluded: Vec<BrainFactId>,
}

async fn owned(h: &Harness, who: &UserId, product: &str, team: &str, scope: FactScope) {
    h.emit(&entered(
        who,
        kind(OWNERSHIP),
        product,
        serde_json::json!({ "team": team }),
        scope,
    ))
    .await;
}

async fn set(h: &Harness, sql: &'static str, id: &BrainFactId) {
    sqlx::query(sql)
        .bind(id.as_str())
        .execute(&h.pg)
        .await
        .expect("age the fact");
}

/// Rebuilds the working-set projection after raw SQL touched the facts;
/// a test that updates scope, topics or status by hand calls this.
pub(crate) async fn refresh_working_set(h: &Harness) {
    let mut conn = h.pg.acquire().await.expect("connection");
    WorkingSetRepository::rebuild_all(&mut conn)
        .await
        .expect("working set rebuild");
}

pub(crate) async fn seed(h: &Harness) -> Seeded {
    let reader = h.user("reader", &[]).await;
    let admin = h.user("admin", &["admin"]).await;
    owned(h, &reader, "zeta", "infra", FactScope::Org).await;
    h.emit(&entered(
        &reader,
        kind(PRICING),
        "acme",
        serde_json::json!({ "price": "1200", "region": "emea" }),
        FactScope::Org,
    ))
    .await;
    owned(
        h,
        &reader,
        "omega",
        "ops",
        FactScope::Project(PROJECT.to_owned()),
    )
    .await;
    owned(h, &reader, "delta", "payments", FactScope::Org).await;
    for product in ["stalex", "retiredx", "disputedx", "duex"] {
        owned(h, &reader, product, "acme", FactScope::Org).await;
    }
    h.drain().await;
    let one = |ids: Vec<BrainFactId>| ids.into_iter().next().expect("seeded fact");
    let pinned = one(h.fact_ids("zeta").await);
    h.service
        .pin(&Harness::caller(&admin, &["admin"]), &pinned)
        .await
        .expect("pin");
    let excluded = vec![
        one(h.fact_ids("stalex").await),
        one(h.fact_ids("retiredx").await),
        one(h.fact_ids("disputedx").await),
        one(h.fact_ids("duex").await),
    ];
    set(
        h,
        "UPDATE brain_facts SET status = 'stale' WHERE id = $1",
        &excluded[0],
    )
    .await;
    set(
        h,
        "UPDATE brain_facts SET status = 'retired' WHERE id = $1",
        &excluded[1],
    )
    .await;
    set(
        h,
        "UPDATE brain_facts SET status = 'disputed' WHERE id = $1",
        &excluded[2],
    )
    .await;
    set(
        h,
        "UPDATE brain_facts SET review_after = now() - interval '1 day' WHERE id = $1",
        &excluded[3],
    )
    .await;
    refresh_working_set(h).await;
    Seeded {
        reader,
        pinned,
        subject: one(h.fact_ids("acme").await),
        context: one(h.fact_ids("omega").await),
        full_text: one(h.fact_ids("delta").await),
        excluded,
    }
}
