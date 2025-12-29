//! Unit tests for AgentEventBus
//!
//! Tests cover:
//! - Event bus creation
//! - Publishing and subscribing to events
//! - Default configuration

use systemprompt_core_agent::{AgentEvent, AgentEventBus};

// ============================================================================
// AgentEventBus Creation Tests
// ============================================================================

#[test]
fn test_event_bus_new() {
    let bus = AgentEventBus::new(100);
    let debug_str = format!("{:?}", bus);
    assert!(debug_str.contains("AgentEventBus"));
}

#[test]
fn test_event_bus_default() {
    let bus = AgentEventBus::default();
    let debug_str = format!("{:?}", bus);
    assert!(debug_str.contains("AgentEventBus"));
}

#[test]
fn test_event_bus_custom_capacity() {
    let bus = AgentEventBus::new(500);
    let debug_str = format!("{:?}", bus);
    assert!(debug_str.contains("AgentEventBus"));
}

// ============================================================================
// Subscribe Tests
// ============================================================================

#[test]
fn test_event_bus_subscribe() {
    let bus = AgentEventBus::new(10);
    let _receiver = bus.subscribe();
}

#[test]
fn test_event_bus_multiple_subscribers() {
    let bus = AgentEventBus::new(10);
    let _receiver1 = bus.subscribe();
    let _receiver2 = bus.subscribe();
    let _receiver3 = bus.subscribe();
}

#[test]
fn test_event_bus_sender() {
    let bus = AgentEventBus::new(10);
    let sender = bus.sender();

    let event = AgentEvent::AgentStartRequested {
        agent_id: "test".to_string(),
    };
    let result = sender.send(event);
    assert!(result.is_err());
}

// ============================================================================
// Publish Tests
// ============================================================================

#[test]
fn test_event_bus_publish_no_subscribers() {
    let bus = AgentEventBus::new(10);

    let event = AgentEvent::AgentStartRequested {
        agent_id: "test".to_string(),
    };

    bus.publish(event);
}

#[tokio::test]
async fn test_event_bus_publish_with_subscriber() {
    let bus = AgentEventBus::new(10);
    let mut receiver = bus.subscribe();

    let event = AgentEvent::AgentStartRequested {
        agent_id: "pub-sub-test".to_string(),
    };

    bus.publish(event);

    let received = receiver.recv().await.unwrap();
    assert_eq!(received.agent_id(), "pub-sub-test");
}

#[tokio::test]
async fn test_event_bus_publish_multiple_events() {
    let bus = AgentEventBus::new(10);
    let mut receiver = bus.subscribe();

    bus.publish(AgentEvent::AgentStartRequested {
        agent_id: "agent-1".to_string(),
    });
    bus.publish(AgentEvent::AgentStartRequested {
        agent_id: "agent-2".to_string(),
    });
    bus.publish(AgentEvent::AgentStartRequested {
        agent_id: "agent-3".to_string(),
    });

    let event1 = receiver.recv().await.unwrap();
    let event2 = receiver.recv().await.unwrap();
    let event3 = receiver.recv().await.unwrap();

    assert_eq!(event1.agent_id(), "agent-1");
    assert_eq!(event2.agent_id(), "agent-2");
    assert_eq!(event3.agent_id(), "agent-3");
}

#[tokio::test]
async fn test_event_bus_broadcast_to_multiple_subscribers() {
    let bus = AgentEventBus::new(10);
    let mut receiver1 = bus.subscribe();
    let mut receiver2 = bus.subscribe();

    let event = AgentEvent::AgentStarted {
        agent_id: "broadcast-test".to_string(),
        pid: 1234,
        port: 8080,
    };

    bus.publish(event);

    let received1 = receiver1.recv().await.unwrap();
    let received2 = receiver2.recv().await.unwrap();

    assert_eq!(received1.agent_id(), "broadcast-test");
    assert_eq!(received2.agent_id(), "broadcast-test");
}

// ============================================================================
// Event Type Tests
// ============================================================================

#[tokio::test]
async fn test_event_bus_different_event_types() {
    let bus = AgentEventBus::new(10);
    let mut receiver = bus.subscribe();

    bus.publish(AgentEvent::AgentStartRequested {
        agent_id: "a1".to_string(),
    });
    bus.publish(AgentEvent::AgentStarted {
        agent_id: "a1".to_string(),
        pid: 100,
        port: 8080,
    });
    bus.publish(AgentEvent::AgentStopped {
        agent_id: "a1".to_string(),
        exit_code: Some(0),
    });

    let e1 = receiver.recv().await.unwrap();
    let e2 = receiver.recv().await.unwrap();
    let e3 = receiver.recv().await.unwrap();

    assert_eq!(e1.event_type(), "agent_start_requested");
    assert_eq!(e2.event_type(), "agent_started");
    assert_eq!(e3.event_type(), "agent_stopped");
}

// ============================================================================
// Debug Implementation Tests
// ============================================================================

#[test]
fn test_event_bus_debug_format() {
    let bus = AgentEventBus::new(50);
    let debug_str = format!("{:?}", bus);

    assert!(debug_str.contains("AgentEventBus"));
    assert!(debug_str.contains("sender"));
}
