use super::*;

pub(super) fn mock_server(
    responses: Vec<(u16, String, String)>,
) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, extra, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let count = stream.read(&mut chunk).unwrap();
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..count]);
                if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length:"))
                        .map(|value| value.trim().parse::<usize>().unwrap())
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            requests.push(String::from_utf8(bytes).unwrap());
            let response = format!(
                "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
        }
        requests
    });
    (format!("http://{address}"), handle)
}

#[tokio::test]
async fn actual_transport_recovers_persisted_retry_and_uses_only_device_credential() {
    let (dir, request) = prepared(EvaluatorClient::Codex);
    let response = ConsumerReceiptResponse {
        receipt_id: InstallationReceiptId::new("retained-receipt"),
        acknowledgement: ReceiptAcknowledgement::IdenticalRetry,
        acknowledged_at: Utc::now(),
        fully_verified: true,
    };
    let (gateway, server) = mock_server(vec![
        (503, String::new(), "{}".to_owned()),
        (
            200,
            String::new(),
            serde_json::to_string(&response).unwrap(),
        ),
    ]);
    let enrollment = Enrollment::new(
        &gateway,
        DeviceId::try_new("device").expect("nonempty fixture device"),
        UserId::new("consumer"),
        systemprompt_bridge::ids::BearerToken::new("sp_device_private"),
    )
    .unwrap();
    let path = enrollment.outbox_path(dir.path());
    let outbox = Outbox::new(path.clone(), OutboxScope::from_enrollment(&enrollment));
    outbox.enqueue(request).unwrap();
    assert!(
        systemprompt_bridge::feedback::deliver(&enrollment, &outbox)
            .await
            .is_err()
    );
    let restarted = Outbox::new(path, OutboxScope::from_enrollment(&enrollment));
    let entry = restarted.entries().unwrap().remove(0).1;
    let wait = (entry.next_attempt - Utc::now())
        .to_std()
        .unwrap_or_default()
        + std::time::Duration::from_millis(20);
    tokio::time::sleep(wait).await;
    systemprompt_bridge::feedback::deliver(&enrollment, &restarted)
        .await
        .unwrap();
    assert!(matches!(
        restarted.entries().unwrap()[0].1.delivery,
        Delivery::Acknowledged(_)
    ));
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests {
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer sp_device_private")
        );
        assert!(!request.contains("per-user-bridge-secret"));
    }
}

#[tokio::test]
async fn enrollment_redirect_is_rejected_without_forwarding_device_credential() {
    let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    target.set_nonblocking(true).unwrap();
    let redirect = format!(
        "Location: http://{}/capture\r\n",
        target.local_addr().unwrap()
    );
    let (gateway, server) = mock_server(vec![(302, redirect, "{}".to_owned())]);
    assert!(matches!(
        systemprompt_bridge::feedback::transport::enroll(&gateway, "sp_device_private").await,
        Err(FeedbackError::Rejected(302))
    ));
    server.join().unwrap();
    assert!(matches!(target.accept(),Err(error) if error.kind()==std::io::ErrorKind::WouldBlock));
}

#[tokio::test]
async fn plan_for_different_host_is_rejected_even_when_publication_and_digest_match() {
    let wrong = plan(EvaluatorClient::Hermes);
    let (gateway, server) = mock_server(vec![(
        200,
        String::new(),
        serde_json::to_string(&wrong).unwrap(),
    )]);
    let enrollment = Enrollment::new(
        &gateway,
        DeviceId::try_new("device").expect("nonempty fixture device"),
        UserId::new("consumer"),
        systemprompt_bridge::ids::BearerToken::new("sp_device_private"),
    )
    .unwrap();
    assert!(matches!(
        systemprompt_bridge::feedback::transport::plan(
            &enrollment,
            &publication(),
            EvaluatorClient::Codex
        )
        .await,
        Err(FeedbackError::Readback)
    ));
    server.join().unwrap();
}

#[tokio::test]
async fn delivery_refuses_different_enrollment_before_any_network_request() {
    let (dir, receipt) = prepared(EvaluatorClient::Codex);
    let outbox = Outbox::new(dir.path().join("outbox.json"), scope("device"));
    outbox.enqueue(receipt).unwrap();
    let other = Enrollment::new(
        "https://example.invalid",
        DeviceId::try_new("other").expect("nonempty fixture device"),
        UserId::new("consumer"),
        systemprompt_bridge::ids::BearerToken::new("sp_device_other"),
    )
    .unwrap();
    assert!(matches!(
        systemprompt_bridge::feedback::deliver(&other, &outbox).await,
        Err(FeedbackError::Scope)
    ));
}

#[test]
fn unchanged_manifest_recovers_pending_plan_but_disabled_or_withdrawn_does_not() {
    let (dir, _) = prepared(EvaluatorClient::Codex);
    temp_env::with_var("XDG_STATE_HOME", Some(dir.path()), || {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            let expected = plan(EvaluatorClient::Codex);
            let (gateway, server) = mock_server(vec![(200, String::new(), serde_json::to_string(&expected).unwrap())]);
            let enrollment = Enrollment::new(&gateway, DeviceId::try_new("device").expect("nonempty fixture device"), UserId::new("consumer"), systemprompt_bridge::ids::BearerToken::new("sp_device_private")).unwrap();
            let outbox = Outbox::new(enrollment.outbox_path(dir.path()), OutboxScope::from_enrollment(&enrollment));
            outbox.reserve_installation(systemprompt_bridge::feedback::outbox::PendingInstallation::new(publication(), EvaluatorClient::Codex, vec![dir.path().to_path_buf()])).unwrap();
            let mut manifest: systemprompt_bridge::gateway::manifest::SignedManifest = serde_json::from_value(serde_json::json!({
                "min_schema_version": 1, "manifest_version": "2026-04-30T12:00:00Z-deadbeef", "issued_at":"2026-04-30T12:00:00Z", "not_before":"2026-04-30T12:00:00Z", "user_id":"consumer", "plugins":[], "skills":[], "managed_mcp_servers":[], "revocations":[], "enabled_hosts":["codex-cli"]
            })).unwrap();
            systemprompt_bridge::feedback::recover_manifest_installations(&enrollment, &outbox, &manifest).await.unwrap();
            assert!(outbox.entries().unwrap().is_empty());
            manifest.skills = vec![serde_json::from_value(serde_json::json!({"id":"skill", "name":"Skill", "description":"", "tags":[], "file_path":"skill/SKILL.md", "sha256":"0".repeat(64), "instructions":"", "publication":publication()})).unwrap()];
            manifest.enabled_hosts.clear();
            systemprompt_bridge::feedback::recover_manifest_installations(&enrollment, &outbox, &manifest).await.unwrap();
            assert!(outbox.entries().unwrap().is_empty());
            manifest.enabled_hosts.push("codex-cli".to_owned());
            systemprompt_bridge::feedback::recover_manifest_installations(&enrollment, &outbox, &manifest).await.unwrap();
            assert_eq!(outbox.entries().unwrap().len(),1);
            assert!(outbox.pending_installations().unwrap().is_empty());
            assert_eq!(server.join().unwrap().len(),1);
        });
    });
}


#[test]
fn feedback_http_errors_remove_request_urls_from_entire_error_chain() {
    let url = "https://example.invalid/private-person?token=private-query-token";
    let raw = reqwest::Client::new()
        .get(url)
        .header("x-invalid", "bad\nvalue")
        .build()
        .expect_err("invalid header fails locally without a network request");
    let raw = raw.with_url(reqwest::Url::parse(url).unwrap());
    assert!(format!("{raw:?}").contains("private-query-token"));
    let error = FeedbackError::from(raw);
    let mut messages = vec![error.to_string(), format!("{error:?}")];
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        messages.push(cause.to_string());
        messages.push(format!("{cause:?}"));
        source = cause.source();
    }
    assert!(
        messages
            .iter()
            .all(|message| !message.contains("private-query-token")
                && !message.contains("private-person"))
    );
    assert!(std::error::Error::source(&error).is_some());
}
