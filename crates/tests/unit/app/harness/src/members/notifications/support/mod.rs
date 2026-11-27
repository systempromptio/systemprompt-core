use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use systemprompt_device::client::{
    CredentialError, CredentialSource, Freshness, HttpClient, SpClient,
};
use systemprompt_device::protocol::Jwt;
use systemprompt_harness::members::notifications::{
    Inbox, KeysetCursor, NotificationsClient, NotificationsHandle,
};
use systemprompt_models::ApiPaths;

mod context;
mod server;

pub(crate) use context::{applied, context, enabled};
pub(crate) use server::{FakeInstance, FakeRequest, FakeResponse};

pub(crate) const USER: &str = "1ecce4e3-bc87-441d-b374-ea29bab88da8";
pub(crate) const BEARER: &str = "Bearer header.claims.signature";

pub(crate) fn http() -> HttpClient {
    HttpClient::new("sp-notifications-tests/0.0.0").unwrap()
}

#[derive(Debug)]
pub(crate) struct StaticJwt;

#[async_trait]
impl CredentialSource for StaticJwt {
    async fn jwt(&self, _freshness: Freshness) -> Result<Jwt, CredentialError> {
        Ok(Jwt::new("header.claims.signature").unwrap())
    }

    async fn rejected(&self) {}
}

pub(crate) fn client(fake: &FakeInstance) -> NotificationsClient {
    let instance = SpClient::new(&fake.url, http()).unwrap();
    NotificationsClient::new(&instance, http(), Arc::new(StaticJwt))
}

pub(crate) fn id_of(index: usize) -> String {
    format!("00000000-0000-4000-8000-{index:012x}")
}

fn base_time() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-20T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

pub(crate) fn created_at(index: usize) -> DateTime<Utc> {
    base_time() - TimeDelta::minutes(i64::try_from(index).unwrap())
}

pub(crate) fn notification(index: usize, kind: &str) -> Value {
    let at = created_at(index).to_rfc3339();
    json!({
        "id": id_of(index),
        "recipient": USER,
        "kind": kind,
        "stream": null,
        "actor": USER,
        "actor_is_agent": false,
        "via_agent": null,
        "subjects": [],
        "attention": "human",
        "attention_reason": "default",
        "action": {
            "prompt": "Open it",
            "open_url": null,
            "document_id": null,
            "comment_id": null,
            "kind": kind,
            "actor": USER
        },
        "title": format!("Item {index}"),
        "body": "",
        "coalesce_count": 1,
        "first_event_at": at,
        "last_event_at": at,
        "created_at": at,
        "read_at": null,
        "escalated_from": null,
        "escalation_note": null,
        "held_until": null,
        "delivery_state": "delivered"
    })
}

/// A scripted feed, newest first, with an optional status every route answers.
#[derive(Debug, Default)]
pub(crate) struct Feed {
    pub(crate) items: Vec<Value>,
    pub(crate) unread: i64,
    pub(crate) fail_with: Option<u16>,
}

pub(crate) type SharedFeed = Arc<Mutex<Feed>>;

pub(crate) fn feed_of(kinds: &[&str], unread: i64) -> SharedFeed {
    let items = kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| notification(index, kind))
        .collect();
    Arc::new(Mutex::new(Feed {
        items,
        unread,
        fail_with: None,
    }))
}

pub(crate) async fn fake_feed(feed: &SharedFeed) -> FakeInstance {
    let feed = Arc::clone(feed);
    FakeInstance::start(move |request| answer(&mut feed.lock().unwrap(), request)).await
}

fn answer(feed: &mut Feed, request: &FakeRequest) -> FakeResponse {
    if let Some(status) = feed.fail_with {
        return FakeResponse::status(status);
    }
    match (request.method.as_str(), request.path()) {
        ("GET", ApiPaths::BRIDGE_NOTIFICATIONS) => page(feed, request),
        ("POST", ApiPaths::BRIDGE_NOTIFICATIONS_READ) => {
            let body = request.json();
            let ids: Vec<Value> = if body["all"] == json!(true) {
                feed.items.iter().map(|item| item["id"].clone()).collect()
            } else {
                body["ids"].as_array().unwrap().clone()
            };
            for item in &mut feed.items {
                if ids.contains(&item["id"]) {
                    item["read_at"] = json!(base_time().to_rfc3339());
                }
            }
            feed.unread = (feed.unread - i64::try_from(ids.len()).unwrap()).max(0);
            FakeResponse::json(200, &json!({ "ids": ids, "unread": feed.unread }))
        },
        ("POST", ApiPaths::BRIDGE_NOTIFICATIONS_SEEN) => {
            let seen = request.json()["ids"].as_array().unwrap().len();
            FakeResponse::json(200, &json!({ "seen": seen }))
        },
        _ => FakeResponse::status(404),
    }
}

fn page(feed: &Feed, request: &FakeRequest) -> FakeResponse {
    let limit: usize = request.query("limit").unwrap().parse().unwrap();
    let start = request.query("since").map_or(0, |raw| {
        let cursor: KeysetCursor = raw.parse().unwrap();
        feed.items
            .iter()
            .position(|item| item["id"] == json!(cursor.id.to_string()))
            .unwrap()
            + 1
    });
    let items: Vec<Value> = feed.items.iter().skip(start).take(limit).cloned().collect();
    let next_cursor = (items.len() == limit).then(|| {
        let last = items.last().unwrap();
        let at = DateTime::parse_from_rfc3339(last["created_at"].as_str().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let id = uuid::Uuid::parse_str(last["id"].as_str().unwrap()).unwrap();
        KeysetCursor::new(at, id).to_string()
    });
    FakeResponse::json(
        200,
        &json!({
            "notifications": items,
            "next_cursor": next_cursor,
            "unread": feed.unread,
            "unread_capped": false
        }),
    )
}

pub(crate) async fn until(
    handle: &NotificationsHandle,
    done: impl Fn(&Inbox) -> bool + Send + Sync,
) -> Inbox {
    let mut changes = handle.subscribe();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let inbox = changes.borrow_and_update().clone();
            if done(&inbox) {
                return inbox;
            }
            changes.changed().await.unwrap();
        }
    })
    .await
    .unwrap()
}
