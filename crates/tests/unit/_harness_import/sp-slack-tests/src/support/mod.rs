use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use sp_client::{CredentialError, CredentialSource, Freshness, HttpClient};
use sp_notifications::Notification;
use sp_protocol::Jwt;
use sp_protocol::wire::paths;

mod context;
mod server;

pub(crate) use context::{applied, context, enabled};
pub(crate) use server::{FakeInstance, FakeRequest, FakeResponse};

const USER: &str = "1ecce4e3-bc87-441d-b374-ea29bab88da8";

pub(crate) fn http() -> HttpClient {
    HttpClient::new("sp-slack-tests/0.0.0").unwrap()
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

pub(crate) fn id_of(index: usize) -> String {
    format!("00000000-0000-4000-8000-{index:012x}")
}

pub(crate) fn notification_json(index: usize, kind: &str, subjects: &[&str]) -> Value {
    let at = format!("2026-11-20T10:{:02}:00Z", 59 - index);
    let subjects: Vec<Value> = subjects
        .iter()
        .map(|kind| json!({ "kind": kind, "id": "C024BE91L" }))
        .collect();
    json!({
        "id": id_of(index),
        "recipient": USER,
        "kind": kind,
        "stream": null,
        "actor": USER,
        "actor_is_agent": false,
        "via_agent": null,
        "subjects": subjects,
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

pub(crate) fn notification(kind: &str, subjects: &[&str]) -> Notification {
    serde_json::from_value(notification_json(0, kind, subjects)).unwrap()
}

/// A scripted feed, newest first, answering one page and acknowledging reads.
pub(crate) type SharedFeed = Arc<Mutex<(Vec<Value>, i64)>>;

pub(crate) fn feed_of(kinds: &[&str]) -> SharedFeed {
    let items: Vec<Value> = kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| notification_json(index, kind, &[]))
        .collect();
    let unread = i64::try_from(items.len()).unwrap();
    Arc::new(Mutex::new((items, unread)))
}

pub(crate) async fn fake_feed(feed: &SharedFeed) -> FakeInstance {
    let feed = Arc::clone(feed);
    FakeInstance::start(move |request| answer(&mut feed.lock().unwrap(), request)).await
}

fn answer(feed: &mut (Vec<Value>, i64), request: &FakeRequest) -> FakeResponse {
    let (items, unread) = feed;
    match (request.method.as_str(), request.path()) {
        ("GET", paths::NOTIFICATIONS) => FakeResponse::json(
            200,
            &json!({
                "notifications": items,
                "next_cursor": null,
                "unread": unread,
                "unread_capped": false
            }),
        ),
        ("POST", paths::NOTIFICATIONS_READ) => {
            let ids = request.json()["ids"].as_array().unwrap().clone();
            for item in items.iter_mut() {
                if ids.contains(&item["id"]) {
                    item["read_at"] = json!("2026-11-20T11:00:00Z");
                }
            }
            *unread -= i64::try_from(ids.len()).unwrap();
            FakeResponse::json(200, &json!({ "ids": ids, "unread": unread }))
        },
        _ => FakeResponse::status(404),
    }
}
