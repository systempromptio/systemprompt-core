use std::time::Duration;

use sp_client::ClientError;
use sp_notifications::{FeedQuery, NotificationId, NotificationsError};
use sp_protocol::wire::notifications::{MarkRead, MarkSeen};
use sp_protocol::wire::paths;

use crate::support::{BEARER, FakeInstance, FakeResponse, client, fake_feed, feed_of, id_of};

#[tokio::test]
async fn the_feed_is_read_with_the_bearer_and_a_clamped_page() {
    let feed = feed_of(&["document.shared", "slack.message"], 2);
    let fake = fake_feed(&feed).await;
    let page = client(&fake)
        .feed(&FeedQuery::newest(500).unread_only())
        .await
        .unwrap();
    assert_eq!(page.notifications.len(), 2);
    assert_eq!(page.unread, 2);
    assert_eq!(page.notifications[0].id.as_str(), id_of(0));
    let request = &fake.to("GET", paths::NOTIFICATIONS)[0];
    assert_eq!(request.header("authorization"), Some(BEARER));
    assert_eq!(request.header("x-systemprompt-client"), Some("harness"));
    assert_eq!(request.query("limit").as_deref(), Some("100"));
    assert_eq!(request.query("unread").as_deref(), Some("true"));
    assert_eq!(request.query("since"), None);
}

#[tokio::test]
async fn read_and_seen_post_their_bodies() {
    let feed = feed_of(&["document.shared", "document.shared"], 2);
    let fake = fake_feed(&feed).await;
    let client = client(&fake);
    let first = NotificationId::try_new(id_of(0)).unwrap();
    let read = client
        .mark_read(&MarkRead {
            ids: vec![first.clone()],
            all: false,
        })
        .await
        .unwrap();
    assert_eq!(read.ids, vec![first.clone()]);
    assert_eq!(read.unread, 1);
    let seen = client
        .mark_seen(&MarkSeen {
            ids: vec![first.clone()],
        })
        .await
        .unwrap();
    assert_eq!(seen.seen, 1);
    let body = fake.to("POST", paths::NOTIFICATIONS_SEEN)[0].json();
    assert_eq!(body, serde_json::json!({ "ids": [first.as_str()] }));
}

#[tokio::test]
async fn an_instance_without_the_feed_is_not_served() {
    let fake = FakeInstance::start(|_| FakeResponse::status(404)).await;
    let error = client(&fake)
        .feed(&FeedQuery::newest(10))
        .await
        .unwrap_err();
    assert!(
        matches!(error, NotificationsError::NotServed { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_refused_token_is_minted_again_once() {
    let fake = FakeInstance::start(|_| FakeResponse::status(401)).await;
    let error = client(&fake)
        .feed(&FeedQuery::newest(10))
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            NotificationsError::Client(ClientError::Unauthorized { .. })
        ),
        "{error:?}"
    );
    assert_eq!(fake.requests().len(), 2);
}

#[tokio::test]
async fn rate_limiting_carries_the_retry_after() {
    let fake = FakeInstance::start(|_| FakeResponse::status(429).header("Retry-After", "7")).await;
    let error = client(&fake)
        .feed(&FeedQuery::newest(10))
        .await
        .unwrap_err();
    assert_eq!(error.retry_after(), Some(Duration::from_secs(7)));
}

#[tokio::test]
async fn a_server_failure_is_unreachable() {
    let fake = FakeInstance::start(|_| FakeResponse::status(503)).await;
    let error = client(&fake)
        .feed(&FeedQuery::newest(10))
        .await
        .unwrap_err();
    assert_eq!(error.kind(), sp_protocol::view::SyncErrorKind::Unreachable);
}
