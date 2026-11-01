//! Agent caps without a database: the order of caps, the level each leaves
//! a user, the capabilities each action exercises, the actor scope and the
//! hourly edit ceiling.

use std::time::{Duration, Instant};

use systemprompt_files::documents::agents::{
    AgentCap, AgentCapability, AgentChannel, AgentRateLimiter, AgentRefusal, DocumentActor,
    RewriteTracker, act_as, current_actor,
};
use systemprompt_identifiers::{AgentName, DocumentId, DocumentNodeId, UserId};
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
    assert_eq!(
        current_actor(),
        DocumentActor::agent(AgentName::new("agent"), false)
    );
    let inside = act_as(DocumentActor::Human, async { current_actor() }).await;
    assert_eq!(inside, DocumentActor::Human);
    let agent = act_as(
        DocumentActor::agent(AgentName::new("claude"), true),
        async { current_actor() },
    )
    .await;
    assert_eq!(agent, DocumentActor::agent(AgentName::new("claude"), true));
    assert_eq!(
        current_actor(),
        DocumentActor::agent(AgentName::new("agent"), false)
    );
}

#[test]
fn the_ceilings_are_per_agent_and_per_user_and_free_after_an_hour() {
    let limiter = AgentRateLimiter::new(3, 5);
    let user = UserId::new("rate-user");
    let start = Instant::now();
    limiter
        .reserve(&user, &AgentName::new("claude"), 2, start)
        .expect("first batch");
    limiter
        .reserve(&user, &AgentName::new("claude"), 1, start)
        .expect("second batch");
    let refused = limiter
        .reserve(&user, &AgentName::new("claude"), 1, start)
        .expect_err("past the agent ceiling");
    assert!(matches!(
        refused,
        AgentRefusal::RateLimited { limit: 3, .. }
    ));
    limiter
        .reserve(&user, &AgentName::new("other-agent"), 2, start)
        .expect("another agent has its own window");
    let total = limiter
        .reserve(&user, &AgentName::new("third-agent"), 1, start)
        .expect_err("past the user's all-agents ceiling");
    assert!(matches!(total, AgentRefusal::RateLimited { limit: 5, .. }));
    limiter
        .reserve(
            &UserId::new("other-user"),
            &AgentName::new("claude"),
            3,
            start,
        )
        .expect("another user has their own windows");
    limiter
        .reserve(
            &user,
            &AgentName::new("claude"),
            3,
            start + Duration::from_secs(3601),
        )
        .expect("the window slid past the first batches");
}

#[test]
fn a_refunded_batch_costs_nothing() {
    let limiter = AgentRateLimiter::new(2, 2);
    let user = UserId::new("refund-user");
    let now = Instant::now();
    for _ in 0..5 {
        let reserved = limiter
            .reserve(&user, &AgentName::new("claude"), 2, now)
            .expect("room");
        limiter.refund(&reserved);
    }
    limiter
        .reserve(&user, &AgentName::new("claude"), 2, now)
        .expect("refunds left the window empty");
}

#[test]
fn rewrites_add_up_across_commits_per_document_and_agent() {
    let tracker = RewriteTracker::new();
    let document = DocumentId::new("00000000-0000-4000-8000-0000000000d0");
    let node = |n: usize| DocumentNodeId::new(format!("body/p[{n}]"));
    let first = [node(1), node(2)];
    let second = [node(2), node(3), node(4)];
    let now = Instant::now();
    assert_eq!(
        tracker.touch(
            &document,
            &AgentName::new("claude"),
            &first.iter().collect(),
            now
        ),
        2
    );
    assert_eq!(
        tracker.touch(
            &document,
            &AgentName::new("claude"),
            &second.iter().collect(),
            now
        ),
        4
    );
    assert_eq!(
        tracker.touch(
            &document,
            &AgentName::new("other-agent"),
            &first.iter().collect(),
            now
        ),
        2,
        "another agent's rewrites are its own"
    );
    let later = now + Duration::from_secs(3601);
    assert_eq!(
        tracker.touch(
            &document,
            &AgentName::new("claude"),
            &first.iter().collect(),
            later
        ),
        2,
        "an hour on, only the new touches count"
    );
}

#[test]
fn a_system_actor_is_never_an_agent_and_channels_mark_only_agents() {
    let system = DocumentActor::system("document_trash_purge");
    assert!(!system.is_agent());
    assert_eq!(system.clone().on(AgentChannel::DriveSync), system);
    let DocumentActor::Agent(agent) =
        DocumentActor::agent(AgentName::new("bridge"), true).on(AgentChannel::DriveSync)
    else {
        panic!("an agent stays an agent");
    };
    assert_eq!(agent.channel, AgentChannel::DriveSync);
}
