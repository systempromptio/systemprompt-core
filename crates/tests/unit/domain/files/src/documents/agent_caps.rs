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
async fn the_actor_is_a_person_outside_a_scope_and_the_agent_inside_one() {
    assert_eq!(current_actor(), DocumentActor::Human);
    let inside = act_as(DocumentActor::agent("claude", false), async {
        current_actor()
    })
    .await;
    assert_eq!(inside, DocumentActor::agent("claude", false));
    assert_eq!(current_actor(), DocumentActor::Human);
}

#[test]
fn the_hourly_ceiling_refuses_past_the_limit_and_frees_after_an_hour() {
    let limiter = AgentRateLimiter::new(3);
    let user = UserId::new("rate-user");
    let start = Instant::now();
    limiter.admit(&user, 2, start).expect("first two");
    limiter.admit(&user, 1, start).expect("third");
    let refused = limiter.admit(&user, 1, start).expect_err("fourth");
    assert!(matches!(
        refused,
        AgentRefusal::RateLimited { limit: 3, .. }
    ));
    limiter
        .admit(&UserId::new("other-user"), 3, start)
        .expect("another user has their own window");
    limiter
        .admit(&user, 3, start + Duration::from_secs(3601))
        .expect("the window slid past the first edits");
}
