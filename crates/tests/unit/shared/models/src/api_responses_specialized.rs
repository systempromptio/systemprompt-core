//! The specialised response envelopes.
//!
//! These are what a client parses, so the assertions are about what reaches
//! the wire: a field that is absent must be omitted rather than sent as null,
//! since a client checking for presence treats an explicit null as a value.
//!
//! The `IntoResponse` impls carry the status codes and the `Location` header,
//! but they are behind the `web` feature, which is off in this crate — testing
//! them here would mean taking an axum dependency for the assertion, so they
//! are left to the api-side suites that already build with the feature on.

use indexmap::IndexMap;
use systemprompt_models::api::responses::{
    AcceptedResponse, CreatedResponse, DiscoveryResponse, Link, SuccessResponse,
};

macro_rules! json {
    ($value:expr) => {
        serde_json::to_value(&$value).expect("serialise response")
    };
}

#[test]
fn a_success_response_carries_its_message() {
    let response = json!(SuccessResponse::new("deleted"));

    assert_eq!(response["message"], "deleted");
    assert!(
        response.get("meta").is_some(),
        "every envelope carries meta so a client can correlate the response"
    );
}

// Why: a 202 without a job is a plain acknowledgement. Sending `job_id: null`
// makes a client that checks for the field's presence believe there is a job
// to poll, and it polls nothing.
#[test]
fn an_accepted_response_without_a_job_omits_the_job_fields_entirely() {
    let response = json!(AcceptedResponse::new("queued"));

    assert_eq!(response["message"], "queued");
    assert!(
        response.get("job_id").is_none(),
        "an absent job must be omitted, not null"
    );
    assert!(response.get("status_url").is_none());
}

// Why: a job id without somewhere to poll is unusable, so the builder sets
// both together. Asserting both appear pins that pairing.
#[test]
fn attaching_a_job_supplies_both_the_id_and_where_to_poll_it() {
    let response = json!(AcceptedResponse::new("queued").with_job("job-1", "/jobs/job-1"));

    assert_eq!(response["job_id"], "job-1");
    assert_eq!(
        response["status_url"], "/jobs/job-1",
        "a job id is only useful alongside its status URL"
    );
}

// Why: `location` is where the created resource now lives. A 201 that omits it
// leaves the client with no way to address what it just made.
#[test]
fn a_created_response_carries_the_location_of_what_was_made() {
    let response = json!(CreatedResponse::new(
        serde_json::json!({"id": "u1"}),
        "/users/u1"
    ));

    assert_eq!(response["location"], "/users/u1");
    assert_eq!(response["data"]["id"], "u1");
}

// Why: a link without a title is still a usable link. Sending an explicit null
// title gives a renderer something to display where there is nothing.
#[test]
fn a_link_without_a_title_omits_it() {
    let titled = json!(Link::new("/next", Some("Next page".to_owned())));
    assert_eq!(titled["title"], "Next page");

    let untitled = json!(Link::new("/next", None));
    assert_eq!(untitled["href"], "/next");
    assert!(untitled.get("title").is_none());
}

// Why: the links block travels under `_links`, not the Rust field name. A
// client following HATEOAS looks for the underscore-prefixed key and finds
// nothing if the rename is dropped.
#[test]
fn discovery_links_travel_under_their_hateoas_key() {
    let mut links = IndexMap::new();
    links.insert("self".to_owned(), Link::new("/here", None));
    links.insert("next".to_owned(), Link::new("/there", None));

    let response = json!(DiscoveryResponse::new(serde_json::json!({}), links));

    assert!(
        response.get("_links").is_some(),
        "the links block must use its wire name: {response}"
    );
    assert!(
        response.get("links").is_none(),
        "the Rust field name must not leak onto the wire"
    );
    assert_eq!(response["_links"]["self"]["href"], "/here");
}

// Why: `IndexMap` is used so link order is the order the caller declared. A
// reordering changes which link a client presenting the first one shows.
#[test]
fn discovery_links_keep_the_order_they_were_inserted() {
    let mut links = IndexMap::new();
    links.insert("first".to_owned(), Link::new("/1", None));
    links.insert("second".to_owned(), Link::new("/2", None));
    links.insert("third".to_owned(), Link::new("/3", None));

    let response = json!(DiscoveryResponse::new(serde_json::json!({}), links));
    let keys: Vec<&str> = response["_links"]
        .as_object()
        .expect("links object")
        .keys()
        .map(String::as_str)
        .collect();

    assert_eq!(keys, vec!["first", "second", "third"]);
}
