use std::sync::Arc;

use systemprompt_identifiers::UserId;
use systemprompt_models::brain::FactScope;
use systemprompt_users::{UserRepository, UserService};

use crate::events::{Landed, entered, price};
use crate::harness::Harness;
use crate::plane::{OWNERSHIP, kind};

async fn enter_owned(h: &Harness, who: &UserId, product: &str) {
    h.emit(&entered(
        who,
        kind(OWNERSHIP),
        product,
        serde_json::json!({ "team": "platform" }),
        FactScope::User(who.clone()),
    ))
    .await;
}

#[tokio::test]
async fn purging_the_owner_removes_their_personal_facts_and_keeps_document_facts_unattributed() {
    let h = Harness::new("brain_purge").await;
    let owner = h.user("owner", &[]).await;
    let bystander = h.user("bystander", &[]).await;
    enter_owned(&h, &owner, "personal").await;
    enter_owned(&h, &bystander, "kept").await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    h.emit(&Landed::new().event(&owner)).await;
    h.drain().await;
    let document_fact = h.only_fact("acme").await;
    h.service
        .confirm(&Harness::caller(&owner, &[]), &document_fact.id)
        .await
        .expect("owner confirms");
    assert_eq!(h.fact_ids("personal").await.len(), 1);

    let users = UserService::new(Arc::new(UserRepository::new(&h.pool)));
    users.delete(&owner).await.expect("purge the owner");

    assert!(
        h.fact_ids("personal").await.is_empty(),
        "the entered fact left"
    );
    let kept = h.fact(&document_fact.id).await;
    assert_eq!(kept.confirmed_by, None);
    assert_eq!(kept.owner_user_id, None);
    assert_eq!(
        h.fact_ids("kept").await.len(),
        1,
        "negative control: another person's entered fact stays"
    );
    h.finish().await;
}
