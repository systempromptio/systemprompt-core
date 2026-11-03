//! An interest's `owner` may be declared as an email: the apply resolves it
//! to the user's id, the runtime reads that id, and an owner no user
//! matches refuses the apply.

use systemprompt_brain::plane::load_declared_state;
use systemprompt_models::brain::InterestDecl;

use crate::harness::Harness;
use crate::plane::{declared, try_apply};

const OWNER_OF: &str = "SELECT COUNT(*) FROM brain_interests WHERE owner_user_id = $1";

fn interest(owner: &str) -> InterestDecl {
    serde_json::from_value(serde_json::json!({
        "id": "renewals",
        "name": "Renewals",
        "owner": owner,
        "question": "which prices changed?",
    }))
    .expect("interest")
}

#[tokio::test]
async fn an_email_owner_resolves_to_the_users_id() {
    let h = Harness::new("brain_interest_owner_email").await;
    let owner = h.user("owner", &[]).await;
    let email = format!("owner-{}@test.invalid", owner.as_str());
    let mut brain = declared();
    brain.interests = vec![interest(&email.to_uppercase())];

    try_apply(&h.pg, &brain)
        .await
        .expect("an email owner applies");

    assert_eq!(h.count(OWNER_OF, owner.as_str()).await, 1);
    let mut conn = h.pg.acquire().await.expect("acquire");
    let state = load_declared_state(&mut conn)
        .await
        .expect("declared state");
    assert_eq!(state.interests[0].owner, owner, "the runtime reads the id");
    h.finish().await;
}

#[tokio::test]
async fn an_owner_no_user_matches_refuses_the_apply() {
    let h = Harness::new("brain_interest_owner_unknown").await;
    let owner = h.user("owner", &[]).await;
    for unknown in ["nobody@test.invalid", "no-such-user-id"] {
        let mut brain = declared();
        brain.interests = vec![interest(unknown)];
        let refused = try_apply(&h.pg, &brain).await;
        assert!(refused.is_err(), "{unknown}");
    }

    let mut brain = declared();
    brain.interests = vec![interest(owner.as_str())];
    try_apply(&h.pg, &brain)
        .await
        .expect("negative control: a user id still applies");
    assert_eq!(h.count(OWNER_OF, owner.as_str()).await, 1);
    h.finish().await;
}
