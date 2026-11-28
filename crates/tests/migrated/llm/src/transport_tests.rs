//! The production HTTP transport against a loopback server: timeouts,
//! redirects, refused connections and identification.

use std::time::Duration;

use systemprompt_engine::llm::retry::{Retry, classify};
use systemprompt_engine::llm::{HttpTransport, LlmError, Timeouts, Transport, TransportError};

use crate::support::{http_response, loopback, post, redirect_loop};

fn short(idle_ms: u64) -> HttpTransport {
    HttpTransport::with_timeouts(Timeouts {
        connect: Duration::from_secs(5),
        idle: Duration::from_millis(idle_ms),
    })
    .unwrap()
}

#[test]
fn the_default_wait_for_data_is_five_minutes() {
    assert_eq!(Timeouts::default().idle, Duration::from_secs(300));
}

#[tokio::test]
async fn headers_that_do_not_arrive_in_time_fail_as_a_retryable_timeout() {
    let server = loopback(vec![String::new()]).await;
    let outcome = short(300)
        .send(post(&format!("{}/v1/messages", server.url)))
        .await;
    let error = outcome.expect_err("the silent server answered");
    assert!(matches!(error, TransportError::Timeout), "{error:?}");
    assert_eq!(classify(&LlmError::Transport(error)), Retry::Backoff);
}

#[tokio::test]
async fn a_refused_connection_is_a_connect_error_that_says_why_and_is_retried() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/messages", listener.local_addr().unwrap());
    drop(listener);
    let error = short(1_000)
        .send(post(&url))
        .await
        .expect_err("a closed port answered");
    assert!(matches!(error, TransportError::Connect(_)), "{error:?}");
    let mut text = error.to_string();
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        text.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    assert!(text.to_lowercase().contains("connect"), "{text}");
    assert_eq!(classify(&LlmError::Transport(error)), Retry::Backoff);
}

#[tokio::test]
async fn a_chain_of_redirects_is_followed_to_its_end() {
    let target = loopback(vec![http_response(200, &[], "done")]).await;
    let hop = loopback(vec![http_response(
        307,
        &[("location", &format!("{}/final", target.url))],
        "",
    )])
    .await;
    let first = loopback(vec![http_response(
        308,
        &[("location", &format!("{}/hop", hop.url))],
        "",
    )])
    .await;
    let response = short(2_000)
        .send(post(&format!("{}/start", first.url)))
        .await
        .unwrap();
    assert_eq!(response.status, 200);
}

#[tokio::test]
async fn a_303_turns_the_post_into_a_get_and_a_307_keeps_it() {
    let mut see_other_target = loopback(vec![http_response(200, &[], "")]).await;
    let see_other = loopback(vec![http_response(
        303,
        &[("location", &format!("{}/x", see_other_target.url))],
        "",
    )])
    .await;
    short(2_000).send(post(&see_other.url)).await.unwrap();
    assert!(
        see_other_target
            .heads
            .recv()
            .await
            .unwrap()
            .starts_with("GET ")
    );
    let mut temporary_target = loopback(vec![http_response(200, &[], "")]).await;
    let temporary = loopback(vec![http_response(
        307,
        &[("location", &format!("{}/x", temporary_target.url))],
        "",
    )])
    .await;
    short(2_000).send(post(&temporary.url)).await.unwrap();
    assert!(
        temporary_target
            .heads
            .recv()
            .await
            .unwrap()
            .starts_with("POST ")
    );
}

#[tokio::test]
async fn credentials_are_dropped_when_a_redirect_changes_origin() {
    let mut target = loopback(vec![http_response(200, &[], "")]).await;
    let other_host = target.url.replace("127.0.0.1", "localhost");
    let start = loopback(vec![http_response(
        307,
        &[("location", &format!("{other_host}/x"))],
        "",
    )])
    .await;
    short(2_000).send(post(&start.url)).await.unwrap();
    let head = target.heads.recv().await.unwrap().to_lowercase();
    assert!(!head.contains("authorization"), "{head}");
}

#[tokio::test]
async fn a_redirect_loop_stops_after_a_bounded_number_of_hops() {
    let looping = redirect_loop(40).await;
    let outcome = short(2_000).send(post(&looping)).await;
    assert!(outcome.is_err());
}

#[tokio::test]
async fn a_redirect_to_a_target_that_cannot_be_followed_is_refused() {
    let start = loopback(vec![http_response(
        307,
        &[("location", "ftp://files.invalid/x")],
        "",
    )])
    .await;
    let outcome = short(2_000).send(post(&start.url)).await;
    match outcome {
        Err(_) => {},
        Ok(response) => assert_eq!(response.status, 307),
    }
}

#[tokio::test]
#[ignore = "parity ledger: model calls follow redirects instead of refusing them"]
async fn a_redirect_from_the_model_endpoint_is_refused() {
    let target = loopback(vec![http_response(200, &[], "")]).await;
    let start = loopback(vec![http_response(
        307,
        &[("location", &format!("{}/x", target.url))],
        "",
    )])
    .await;
    let response = short(2_000).send(post(&start.url)).await.unwrap();
    assert_eq!(response.status, 307);
}

#[tokio::test]
#[ignore = "parity ledger: model calls carry no user-agent naming the application and its version"]
async fn model_calls_identify_the_application_and_its_version() {
    let mut server = loopback(vec![http_response(200, &[], "")]).await;
    short(2_000).send(post(&server.url)).await.unwrap();
    let head = server.heads.recv().await.unwrap().to_lowercase();
    assert!(head.contains("user-agent: harness/"), "{head}");
}

#[test]
fn the_fetch_tool_identifies_the_application_and_its_version() {
    let config = systemprompt_engine::tools::FetchConfig::default();
    let agent = &config.user_agent;
    let (name, version) = agent.split_once('/').unwrap();
    assert_eq!(name, "harness");
    assert!(
        !version.is_empty() && version.chars().next().is_some_and(|c| c.is_ascii_digit()),
        "{agent}"
    );
}
