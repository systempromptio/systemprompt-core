use serde_json::json;
use systemprompt_models::ApiPaths;
use systemprompt_models::notifications::feed::{FEED_PAGE_MAX, SeenAnswer};
use systemprompt_models::notifications::{
    Attention, MarkRead, MarkSeen, NotificationPage, NotificationReadPayload,
};

const ID: &str = "0b9f8a52-6d1e-4c3f-9a7b-2e4d6f8a0c1e";
const USER: &str = "1ecce4e3-bc87-441d-b374-ea29bab88da8";

fn notification() -> serde_json::Value {
    json!({
        "id": ID,
        "recipient": USER,
        "kind": "slack.message",
        "stream": null,
        "actor": USER,
        "actor_is_agent": false,
        "via_agent": null,
        "subjects": [{ "kind": "slack", "id": "C024BE91L" }],
        "attention": "human",
        "attention_reason": "default",
        "action": {
            "prompt": "Reply in the thread",
            "open_url": null,
            "document_id": null,
            "comment_id": null,
            "kind": "slack.message",
            "actor": USER
        },
        "title": "New message",
        "body": "Can you look at this?",
        "coalesce_count": 1,
        "first_event_at": "2026-11-20T10:00:00Z",
        "last_event_at": "2026-11-20T10:00:00Z",
        "created_at": "2026-11-20T10:00:00Z",
        "read_at": null,
        "escalated_from": null,
        "escalation_note": null,
        "held_until": null,
        "delivery_state": "delivered"
    })
}

#[test]
fn feed_routes_are_the_bridge_notification_routes() {
    assert_eq!(ApiPaths::BRIDGE_NOTIFICATIONS, "/v1/bridge/notifications");
    assert_eq!(
        ApiPaths::BRIDGE_NOTIFICATIONS_READ,
        "/v1/bridge/notifications/read"
    );
    assert_eq!(
        ApiPaths::BRIDGE_NOTIFICATIONS_SEEN,
        "/v1/bridge/notifications/seen"
    );
    assert_eq!(FEED_PAGE_MAX, 100);
}

#[test]
fn a_feed_page_decodes_with_its_unread_count() {
    let page: NotificationPage = serde_json::from_value(json!({
        "notifications": [notification()],
        "next_cursor": null,
        "unread": 3,
        "unread_capped": false
    }))
    .unwrap();
    assert_eq!(page.unread, 3);
    assert!(page.next_cursor.is_none());
    let item = &page.notifications[0];
    assert_eq!(item.id.as_str(), ID);
    assert_eq!(item.kind.as_str(), "slack.message");
    assert_eq!(item.attention, Attention::Human);
    assert_eq!(item.subjects[0].kind, "slack");
}

#[test]
fn acknowledgement_bodies_match_the_instance() {
    let read = serde_json::to_value(MarkRead {
        ids: Vec::new(),
        all: true,
    })
    .unwrap();
    assert_eq!(read, json!({ "ids": [], "all": true }));
    let seen = serde_json::to_value(MarkSeen { ids: Vec::new() }).unwrap();
    assert_eq!(seen, json!({ "ids": [] }));
    let answer: NotificationReadPayload =
        serde_json::from_value(json!({ "ids": [ID], "unread": 0 })).unwrap();
    assert_eq!(answer.ids.len(), 1);
    let seen: SeenAnswer = serde_json::from_value(json!({ "seen": 2 })).unwrap();
    assert_eq!(seen, SeenAnswer { seen: 2 });
}
