//! Bridge notification routes and `count_tokens` through the gateway router:
//! every route refuses a caller without a bridge credential, and a signed-in
//! user reads and acknowledges an empty feed and gets a token estimate.

use axum::http::StatusCode;

use super::gateway_provider_credentials::{request, send, token};

#[tokio::test]
async fn every_bridge_notification_route_refuses_a_missing_credential() -> anyhow::Result<()> {
    for (method, uri, body) in [
        ("GET", "/bridge/notifications", None),
        (
            "POST",
            "/bridge/notifications/read",
            Some(r#"{"all":true}"#),
        ),
        ("POST", "/bridge/notifications/seen", Some(r#"{"ids":[]}"#)),
        ("POST", "/bridge/notifications/n1/escalate", Some("{}")),
        ("GET", "/bridge/notifications/streams", None),
        ("GET", "/bridge/notifications/subscriptions", None),
        ("DELETE", "/bridge/notifications/subscriptions/s1", None),
    ] {
        let (status, body) = send(request(method, uri, None, body.map(str::to_owned))).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}: {body}");
    }
    Ok(())
}

#[tokio::test]
async fn a_bridge_user_reads_and_acknowledges_an_empty_feed() -> anyhow::Result<()> {
    let token = token("bridge-notify@example.invalid").await?;
    let auth = Some(token.as_str());

    let (status, body) = send(request("GET", "/bridge/notifications?limit=5", auth, None)).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let page: serde_json::Value = serde_json::from_str(&body)?;
    assert!(page.is_object(), "{body}");

    let (status, body) = send(request(
        "POST",
        "/bridge/notifications/read",
        auth,
        Some(r#"{"all":true}"#.to_owned()),
    ))
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = send(request(
        "POST",
        "/bridge/notifications/seen",
        auth,
        Some(r#"{"ids":[]}"#.to_owned()),
    ))
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");

    for uri in [
        "/bridge/notifications/streams",
        "/bridge/notifications/subscriptions",
    ] {
        let (status, body) = send(request("GET", uri, auth, None)).await?;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
    }

    let unknown = uuid::Uuid::new_v4().to_string();
    let (status, body) = send(request(
        "POST",
        &format!("/bridge/notifications/{unknown}/escalate"),
        auth,
        Some(r#"{"note":"look"}"#.to_owned()),
    ))
    .await?;
    assert!(status.is_client_error(), "{status}: {body}");
    let (status, body) = send(request(
        "DELETE",
        &format!("/bridge/notifications/subscriptions/{unknown}"),
        auth,
        None,
    ))
    .await?;
    assert!(status.is_client_error(), "{status}: {body}");
    let (status, _) = send(request(
        "POST",
        "/bridge/notifications/subscriptions",
        auth,
        Some(r#"{"subject":"nonsense"}"#.to_owned()),
    ))
    .await?;
    assert!(status.is_client_error());
    Ok(())
}

#[tokio::test]
async fn count_tokens_estimates_for_an_authenticated_caller_only() -> anyhow::Result<()> {
    let body = serde_json::json!({
        "model": "byok-model-1",
        "system": "abcd",
        "messages": [{"role": "user", "content": [{"type": "text", "text": "abcdefgh"}]}],
        "tools": [{"name": "t", "description": "xyz"}],
    })
    .to_string();
    let (status, _) = send(request(
        "POST",
        "/messages/count_tokens",
        None,
        Some(body.clone()),
    ))
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(request(
        "POST",
        "/messages/count_tokens",
        Some("forged"),
        Some(body.clone()),
    ))
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let token = token("count-tokens@example.invalid").await?;
    let (status, reply) = send(request(
        "POST",
        "/messages/count_tokens",
        Some(&token),
        Some(body),
    ))
    .await?;
    assert_eq!(status, StatusCode::OK, "{reply}");
    let value: serde_json::Value = serde_json::from_str(&reply)?;
    let expected = systemprompt_api::routes::gateway::count_tokens::estimate_input_tokens(
        serde_json::json!({
            "system": "abcd",
            "messages": [{"role": "user", "content": [{"type": "text", "text": "abcdefgh"}]}],
            "tools": [{"name": "t", "description": "xyz"}],
        })
        .to_string()
        .as_bytes(),
    );
    assert_eq!(value["input_tokens"], expected);
    assert!(expected >= 4);

    let (status, reply) = send(request(
        "POST",
        "/messages/count_tokens",
        Some(&token),
        Some("not json".to_owned()),
    ))
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(reply.contains("\"input_tokens\":0"), "{reply}");
    Ok(())
}
