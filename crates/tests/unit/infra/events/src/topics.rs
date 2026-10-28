use std::sync::Arc;

use systemprompt_events::{EventBroadcasters, EventRouter, TopicBroadcaster, TopicEvent};
use systemprompt_identifiers::UserId;

fn event(topic: &str, name: &str) -> TopicEvent {
    TopicEvent {
        topic: topic.to_owned(),
        name: name.to_owned(),
        resume_id: None,
        value: serde_json::json!({ "n": name }),
    }
}

#[test]
fn publish_without_subscribers_reaches_nobody() {
    let topics = TopicBroadcaster::new();
    assert_eq!(topics.publish(event("document:a", "x")), 0);
}

#[tokio::test]
async fn every_subscriber_of_a_topic_receives_it_and_no_other_topic_does() {
    let topics = TopicBroadcaster::new();
    let mut first = topics.subscribe("document:a");
    let mut second = topics.subscribe("document:a");
    let mut other = topics.subscribe("document:b");
    assert_eq!(topics.publish(event("document:a", "x")), 2);
    assert_eq!(first.recv().await.expect("first").name, "x");
    assert_eq!(second.recv().await.expect("second").name, "x");
    assert!(other.try_recv().is_err());
}

#[test]
fn a_topic_is_dropped_once_its_last_subscriber_leaves() {
    let topics = TopicBroadcaster::new();
    let receiver = topics.subscribe("document:a");
    assert_eq!(topics.subscriber_count("document:a"), 1);
    drop(receiver);
    assert_eq!(topics.publish(event("document:a", "x")), 0);
    assert_eq!(topics.subscriber_count("document:a"), 0);
}

#[tokio::test]
async fn a_local_router_publishes_topics_without_relaying() {
    let broadcasters = Arc::new(EventBroadcasters::new());
    let router = EventRouter::local_only(Arc::clone(&broadcasters));
    let mut receiver = broadcasters.topics.subscribe("document:a");
    let outcome = router
        .route_topic(&UserId::new("user-1"), event("document:a", "y"))
        .await;
    assert_eq!(outcome.local, 1);
    assert!(matches!(
        outcome.relay,
        systemprompt_events::RelayOutcome::NotInstalled
    ));
    assert_eq!(receiver.recv().await.expect("event").name, "y");
}
