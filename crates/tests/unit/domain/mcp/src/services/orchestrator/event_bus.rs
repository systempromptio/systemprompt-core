//! Unit tests for MCP EventBus

use systemprompt_core_mcp::services::orchestrator::{EventBus, McpEvent};

// ============================================================================
// EventBus Creation Tests
// ============================================================================

#[test]
fn test_event_bus_new() {
    let event_bus = EventBus::new(100);
    let debug_str = format!("{:?}", event_bus);
    assert!(debug_str.contains("EventBus"));
    assert!(debug_str.contains("handlers_count"));
}

#[test]
#[should_panic(expected = "broadcast channel capacity cannot be zero")]
fn test_event_bus_new_with_zero_capacity_panics() {
    // Zero capacity is not allowed for broadcast channels
    let _event_bus = EventBus::new(0);
}

#[test]
fn test_event_bus_new_with_large_capacity() {
    let event_bus = EventBus::new(10000);
    let debug_str = format!("{:?}", event_bus);
    assert!(debug_str.contains("EventBus"));
}

// ============================================================================
// EventBus Subscribe Tests
// ============================================================================

#[test]
fn test_event_bus_subscribe() {
    let event_bus = EventBus::new(100);
    let _receiver = event_bus.subscribe();
}

#[test]
fn test_event_bus_multiple_subscribers() {
    let event_bus = EventBus::new(100);
    let _receiver1 = event_bus.subscribe();
    let _receiver2 = event_bus.subscribe();
    let _receiver3 = event_bus.subscribe();
}

// ============================================================================
// EventBus Sender Tests
// ============================================================================

#[test]
fn test_event_bus_sender() {
    let event_bus = EventBus::new(100);
    let sender = event_bus.sender();

    // Verify sender can be cloned
    let _sender_clone = sender.clone();
}

#[test]
fn test_event_bus_sender_send() {
    let event_bus = EventBus::new(100);
    let sender = event_bus.sender();
    let mut receiver = event_bus.subscribe();

    let event = McpEvent::ServiceStartRequested {
        service_name: "test-service".to_string(),
    };

    let _ = sender.send(event);

    // Try to receive (may or may not succeed depending on timing)
    let _ = receiver.try_recv();
}

// ============================================================================
// EventBus Publish Tests
// ============================================================================

#[tokio::test]
async fn test_event_bus_publish() {
    let event_bus = EventBus::new(100);
    let mut receiver = event_bus.subscribe();

    let event = McpEvent::ServiceStarted {
        service_name: "test-service".to_string(),
        process_id: 1234,
        port: 8080,
    };

    let result = event_bus.publish(event).await;
    assert!(result.is_ok());

    // Verify event was received
    let received = receiver.try_recv();
    assert!(received.is_ok());
    assert_eq!(received.unwrap().service_name(), "test-service");
}

#[tokio::test]
async fn test_event_bus_publish_multiple_events() {
    let event_bus = EventBus::new(100);
    let mut receiver = event_bus.subscribe();

    let events = vec![
        McpEvent::ServiceStartRequested {
            service_name: "service1".to_string(),
        },
        McpEvent::ServiceStarted {
            service_name: "service1".to_string(),
            process_id: 1234,
            port: 8080,
        },
        McpEvent::ServiceStopped {
            service_name: "service1".to_string(),
            exit_code: Some(0),
        },
    ];

    for event in events {
        let result = event_bus.publish(event).await;
        assert!(result.is_ok());
    }

    // Verify all events were received
    let mut received_count = 0;
    while receiver.try_recv().is_ok() {
        received_count += 1;
    }
    assert_eq!(received_count, 3);
}

#[tokio::test]
async fn test_event_bus_publish_without_subscribers() {
    let event_bus = EventBus::new(100);

    let event = McpEvent::ServiceStarted {
        service_name: "test-service".to_string(),
        process_id: 1234,
        port: 8080,
    };

    // Should not error even without subscribers
    let result = event_bus.publish(event).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_event_bus_publish_to_multiple_subscribers() {
    let event_bus = EventBus::new(100);
    let mut receiver1 = event_bus.subscribe();
    let mut receiver2 = event_bus.subscribe();

    let event = McpEvent::ServiceStarted {
        service_name: "test-service".to_string(),
        process_id: 1234,
        port: 8080,
    };

    let result = event_bus.publish(event).await;
    assert!(result.is_ok());

    // Both receivers should receive the event
    assert!(receiver1.try_recv().is_ok());
    assert!(receiver2.try_recv().is_ok());
}

// ============================================================================
// EventBus Debug Tests
// ============================================================================

#[test]
fn test_event_bus_debug_format() {
    let event_bus = EventBus::new(100);
    let debug_str = format!("{:?}", event_bus);

    assert!(debug_str.contains("EventBus"));
    assert!(debug_str.contains("handlers_count"));
    assert!(debug_str.contains("0")); // Initially 0 handlers
}
