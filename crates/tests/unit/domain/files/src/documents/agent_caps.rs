//! Agent caps without a database: the order of caps, the level each leaves
//! a user, the capabilities each action exercises, the actor scope and the
//! hourly edit ceiling.

use std::time::{Duration, Instant};

use systemprompt_files::documents::agents::{
    AgentCap, AgentCapability, AgentRateLimiter, AgentRefusal, DocumentActor, act_as, current_actor,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::documents::{AccessLevel, DocumentAction};

#[test]
fn caps_order_from_strictest() {
    assert!(AgentCap::None < AgentCap::Read);
    assert!(AgentCap::Read < AgentCap::Comment);
    assert!(AgentCap::Comment < AgentCap::Suggest);
    assert!(AgentCap::Suggest < AgentCap::Edit);
    assert_eq!(AgentCap::Edit.min(AgentCap::Read), AgentCap::Read);
}

#[test]
fn the_cap_matrix_lowers_the_users_level_and_never_raises_it() {
    let levels = [
        AccessLevel::View,
        AccessLevel::Comment,
        AccessLevel::Edit,
        AccessLevel::Share,
        AccessLevel::Own,
    ];
    for held in levels {
        assert_eq!(AgentCap::None.cap_level(held), None);
        assert_eq!(AgentCap::Read.cap_level(held), Some(AccessLevel::View));
        let comment = Some(held.min(AccessLevel::Comment));
        assert_eq!(AgentCap::Comment.cap_level(held), comment);
        assert_eq!(AgentCap::Suggest.cap_level(held), comment);
        assert_eq!(AgentCap::Edit.cap_level(held), Some(held));
    }
}

#[test]
fn caps_round_trip_their_wire_names() {
    for cap in AgentCap::ALL {
        assert_eq!(cap.as_str().parse::<AgentCap>().expect("parse"), cap);
    }
    assert!("admin".parse::<AgentCap>().is_err());
}

#[test]
fn only_the_actions_above_edit_are_capabilities() {
    assert_eq!(
        AgentCapability::of_action(DocumentAction::Share),
        Some(AgentCapability::Share)
    );
    assert_eq!(
        AgentCapability::of_action(DocumentAction::Delete),
        Some(AgentCapability::Trash)
    );
    assert_eq!(
        AgentCapability::of_action(DocumentAction::Own),
        Some(AgentCapability::Administer)
    );
    for action in [
        DocumentAction::View,
        DocumentAction::Comment,
        DocumentAction::Edit,
        DocumentAction::Create,
    ] {
        assert_eq!(AgentCapability::of_action(action), None);
    }
}

#[tokio::test]
async fn outside_a_scope_the_actor_is_an_unnamed_unattended_agent() {
    assert_eq!(current_actor(), DocumentActor::agent("agent", false));
    let inside = act_as(DocumentActor::Human, async { current_actor() }).await;
    assert_eq!(inside, DocumentActor::Human);
    let agent = act_as(DocumentActor::agent("claude", true), async {
        current_actor()
    })
    .await;
    assert_eq!(agent, DocumentActor::agent("claude", true));
    assert_eq!(current_actor(), DocumentActor::agent("agent", false));
}

#[test]
fn the_hourly_ceiling_is_per_user_and_agent_and_frees_after_an_hour() {
    let limiter = AgentRateLimiter::new(3);
    let user = UserId::new("rate-user");
    let start = Instant::now();
    limiter
        .admit(&user, "claude", 2, start)
        .expect("first batch");
    limiter
        .admit(&user, "claude", 1, start)
        .expect("second batch");
    let refused = limiter
        .check(&user, "claude", 1, start)
        .expect_err("past the ceiling");
    assert!(matches!(
        refused,
        AgentRefusal::RateLimited { limit: 3, .. }
    ));
    limiter
        .admit(&user, "other-agent", 3, start)
        .expect("another agent has its own window");
    limiter
        .admit(&UserId::new("other-user"), "claude", 3, start)
        .expect("another user has their own window");
    limiter
        .admit(&user, "claude", 3, start + Duration::from_secs(3601))
        .expect("the window slid past the first batches");
}

#[test]
fn a_checked_batch_that_never_lands_costs_nothing() {
    let limiter = AgentRateLimiter::new(2);
    let user = UserId::new("unlanded-user");
    let now = Instant::now();
    for _ in 0..5 {
        limiter
            .check(&user, "claude", 2, now)
            .expect("nothing recorded");
    }
    assert_eq!(limiter.tracked(), 0);
}
