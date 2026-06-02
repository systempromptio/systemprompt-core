//! `HealthCheckFailed` trips a bounded `ServiceRestartRequested` once
//! failures cross the threshold (N = 3) and resets on `ServiceStarted` —
//! never an infinite retry.

use std::sync::Arc;
use std::time::Duration;
use systemprompt_mcp::services::orchestrator::handlers::{EventHandler, HealthCheckHandler};
use systemprompt_mcp::services::{EventBus as McpEventBus, McpEvent};
use tokio::sync::broadcast::error::TryRecvError;
use tokio::time::timeout;

fn build_bus_with_health_check() -> (McpEventBus, tokio::sync::broadcast::Receiver<McpEvent>) {
    let mut bus = McpEventBus::new(64);
    let sender = bus.sender();
    let handler = HealthCheckHandler::new().with_restart_sender(sender);
    bus.register_handler(Arc::new(handler) as Arc<dyn EventHandler>);
    let rx = bus.subscribe();
    (bus, rx)
}

async fn drain_until_restart(
    rx: &mut tokio::sync::broadcast::Receiver<McpEvent>,
    service_name: &str,
) -> Option<McpEvent> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        match timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Ok(evt @ McpEvent::ServiceRestartRequested { .. }))
                if evt.service_name() == service_name =>
            {
                return Some(evt);
            },
            Ok(Ok(_)) => continue,
            Ok(Err(_)) | Err(_) => continue,
        }
    }
}

#[tokio::test]
async fn health_check_below_threshold_does_not_request_restart() {
    let (bus, mut rx) = build_bus_with_health_check();

    for _ in 0..2 {
        bus.publish(McpEvent::HealthCheckFailed {
            service_name: "alpha".to_owned(),
            reason: "503".to_owned(),
        })
        .await
        .expect("publish");
    }

    for _ in 0..2 {
        let _ = rx.recv().await;
    }

    match rx.try_recv() {
        Err(TryRecvError::Empty) => {},
        other => panic!("expected empty bus, got {other:?}"),
    }
}

#[tokio::test]
async fn health_check_at_threshold_emits_one_restart_request() {
    let (bus, mut rx) = build_bus_with_health_check();

    for _ in 0..3 {
        bus.publish(McpEvent::HealthCheckFailed {
            service_name: "bravo".to_owned(),
            reason: "503".to_owned(),
        })
        .await
        .expect("publish");
    }

    let restart = drain_until_restart(&mut rx, "bravo")
        .await
        .expect("restart request must fire at the 3rd failure");

    match restart {
        McpEvent::ServiceRestartRequested {
            service_name,
            reason,
        } => {
            assert_eq!(service_name, "bravo");
            assert!(
                reason.contains("3"),
                "restart reason must reference the failure count, got {reason:?}"
            );
        },
        other => panic!("unexpected event {other:?}"),
    }
}

#[tokio::test]
async fn health_check_does_not_request_restart_in_infinite_loop() {
    let (bus, mut rx) = build_bus_with_health_check();

    for _ in 0..10 {
        bus.publish(McpEvent::HealthCheckFailed {
            service_name: "charlie".to_owned(),
            reason: "503".to_owned(),
        })
        .await
        .expect("publish");
    }

    let mut restart_count = 0usize;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    while tokio::time::Instant::now() < deadline {
        match timeout(Duration::from_millis(50), rx.recv()).await {
            Ok(Ok(McpEvent::ServiceRestartRequested { service_name, .. }))
                if service_name == "charlie" =>
            {
                restart_count += 1;
            },
            Ok(Ok(_)) | Ok(Err(_)) | Err(_) => {},
        }
    }

    assert!(
        restart_count >= 1,
        "at least one restart request must fire on a 10-failure storm, got {restart_count}"
    );
    assert!(
        restart_count <= 10,
        "restart fires must be bounded by failure count, got {restart_count}"
    );
}

#[tokio::test]
async fn service_started_event_resets_failure_counter() {
    let (bus, mut rx) = build_bus_with_health_check();

    for _ in 0..2 {
        bus.publish(McpEvent::HealthCheckFailed {
            service_name: "delta".to_owned(),
            reason: "503".to_owned(),
        })
        .await
        .expect("publish");
    }

    bus.publish(McpEvent::ServiceStarted {
        service_name: "delta".to_owned(),
        process_id: 1,
        port: 1,
    })
    .await
    .expect("publish");

    for _ in 0..2 {
        bus.publish(McpEvent::HealthCheckFailed {
            service_name: "delta".to_owned(),
            reason: "503".to_owned(),
        })
        .await
        .expect("publish");
    }

    let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Ok(McpEvent::ServiceRestartRequested { service_name, .. })) =
            timeout(Duration::from_millis(50), rx.recv()).await
        {
            if service_name == "delta" {
                panic!("counter not reset after ServiceStarted");
            }
        }
    }
}
