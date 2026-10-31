//! The in-memory agent windows at their bounds: a batch larger than a
//! ceiling, the retry hint, eviction past `MAX_TRACKED` keys, and rewrites
//! weighed before they are recorded.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use systemprompt_files::documents::agents::{
    AgentRateLimiter, AgentRefusal, MAX_TRACKED, RewriteTracker,
};
use systemprompt_identifiers::{AgentName, DocumentId, DocumentNodeId, UserId};

#[test]
fn a_first_batch_larger_than_the_ceiling_waits_a_full_hour() {
    let limiter = AgentRateLimiter::new(3, 1);
    let refused = limiter
        .reserve(
            &UserId::new("big-user"),
            &AgentName::new("claude"),
            4,
            Instant::now(),
        )
        .expect_err("over the agent ceiling");
    assert_eq!(
        refused,
        AgentRefusal::RateLimited {
            limit: 3,
            retry_after_secs: 3600
        }
    );
    assert_eq!(limiter.tracked(), 0, "a refusal reserves nothing");
}

#[test]
fn a_zero_op_batch_counts_as_one_and_the_retry_hint_tracks_the_oldest_batch() {
    let limiter = AgentRateLimiter::new(2, 2);
    let user = UserId::new("hint-user");
    let agent = AgentName::new("claude");
    let start = Instant::now();
    limiter.reserve(&user, &agent, 0, start).expect("one");
    limiter
        .reserve(&user, &agent, 0, start + Duration::from_secs(600))
        .expect("two");
    let refused = limiter
        .reserve(&user, &agent, 1, start + Duration::from_secs(1200))
        .expect_err("full");
    assert_eq!(
        refused,
        AgentRefusal::RateLimited {
            limit: 2,
            retry_after_secs: 2400
        }
    );
    limiter
        .reserve(&user, &agent, 1, start + Duration::from_secs(3600))
        .expect("oldest batch expired");
}

#[test]
fn the_limiter_evicts_spent_windows_then_the_least_recently_used() {
    let limiter = AgentRateLimiter::new(1, 1);
    let agent = AgentName::new("a");
    let start = Instant::now();
    for n in 0..MAX_TRACKED / 2 {
        limiter
            .reserve(&UserId::new(format!("user-{n}")), &agent, 1, start)
            .expect("room");
    }
    assert_eq!(limiter.tracked(), MAX_TRACKED);
    let later = start + Duration::from_secs(3601);
    limiter
        .reserve(&UserId::new("newcomer"), &agent, 1, later)
        .expect("spent windows make room");
    assert_eq!(limiter.tracked(), 2);

    let full = AgentRateLimiter::new(1, 1);
    for n in 0..MAX_TRACKED / 2 {
        full.reserve(
            &UserId::new(format!("live-{n}")),
            &agent,
            1,
            start + Duration::from_millis(n as u64),
        )
        .expect("room");
    }
    full.reserve(
        &UserId::new("late"),
        &agent,
        1,
        start + Duration::from_secs(60),
    )
    .expect("least recently used evicted");
    assert_eq!(full.tracked(), MAX_TRACKED);
    full.reserve(
        &UserId::new("live-0"),
        &agent,
        1,
        start + Duration::from_secs(61),
    )
    .expect("the oldest user's windows were evicted, so it starts afresh");
}

#[test]
fn rewrites_are_weighed_before_they_are_recorded() {
    let tracker = RewriteTracker::new();
    let document = DocumentId::new("00000000-0000-4000-8000-0000000000d1");
    let agent = AgentName::new("claude");
    let a = DocumentNodeId::new("body/p[1]");
    let b = DocumentNodeId::new("body/p[2]");
    let now = Instant::now();
    let first: HashSet<&DocumentNodeId> = [&a].into_iter().collect();
    let both: HashSet<&DocumentNodeId> = [&a, &b].into_iter().collect();
    assert_eq!(tracker.would_reach(&document, &agent, &both, now), 2);
    tracker.touch(&document, &agent, &first, now);
    assert_eq!(
        tracker.would_reach(&document, &agent, &first, now),
        1,
        "weighing does not record"
    );
    assert_eq!(tracker.would_reach(&document, &agent, &both, now), 2);
    let later = now + Duration::from_secs(3600);
    assert_eq!(
        tracker.would_reach(&document, &agent, &HashSet::new(), later),
        0,
        "an hour on the earlier touch has expired"
    );
}

#[test]
fn the_rewrite_tracker_evicts_expired_then_least_recently_used_documents() {
    let tracker = RewriteTracker::new();
    let agent = AgentName::new("claude");
    let node = DocumentNodeId::new("body/p[1]");
    let nodes: HashSet<&DocumentNodeId> = [&node].into_iter().collect();
    let start = Instant::now();
    let doc = |n: usize| DocumentId::new(format!("00000000-0000-4000-8000-{n:012}"));
    for n in 0..MAX_TRACKED {
        tracker.touch(
            &doc(n),
            &agent,
            &nodes,
            start + Duration::from_millis(n as u64),
        );
    }
    let soon = start + Duration::from_secs(60);
    tracker.touch(&doc(MAX_TRACKED), &agent, &nodes, soon);
    assert_eq!(
        tracker.would_reach(&doc(0), &agent, &HashSet::new(), soon),
        0,
        "the least recently used document was evicted"
    );
    assert_eq!(
        tracker.would_reach(&doc(1), &agent, &HashSet::new(), soon),
        1
    );
    let later = start + Duration::from_secs(7200);
    tracker.touch(&doc(MAX_TRACKED + 1), &agent, &nodes, later);
    assert_eq!(
        tracker.would_reach(&doc(MAX_TRACKED + 1), &agent, &HashSet::new(), later),
        1
    );
    assert_eq!(tracker.touch(&doc(1), &agent, &nodes, later), 1);
}
