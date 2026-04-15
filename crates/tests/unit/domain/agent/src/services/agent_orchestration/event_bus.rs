use systemprompt_agent::{AgentEvent, AgentEventBus};
use systemprompt_identifiers::AgentId;

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

#[test]
fn test_event_bus_sender() {
    let bus = AgentEventBus::new(10);
    let sender = bus.sender();

    let event = AgentEvent::AgentStartRequested {
        agent_id: AgentId::new("test"),
    };
    let result = sender.send(event);
    result.unwrap_err();
}

#[tokio::test]
async fn test_event_bus_publish_with_subscriber() {
    let bus = AgentEventBus::new(10);
    let mut receiver = bus.subscribe();

    let event = AgentEvent::AgentStartRequested {
        agent_id: AgentId::new("pub-sub-test"),
    };

    bus.publish(event);

    let received = receiver.recv().await.unwrap();
    assert_eq!(received.agent_id().map(|a| a.as_str()), Some("pub-sub-test"));
}

#[tokio::test]
async fn test_event_bus_publish_multiple_events() {
    let bus = AgentEventBus::new(10);
    let mut receiver = bus.subscribe();

    bus.publish(AgentEvent::AgentStartRequested {
        agent_id: AgentId::new("agent-1"),
    });
    bus.publish(AgentEvent::AgentStartRequested {
        agent_id: AgentId::new("agent-2"),
    });
    bus.publish(AgentEvent::AgentStartRequested {
        agent_id: AgentId::new("agent-3"),
    });

    let event1 = receiver.recv().await.unwrap();
    let event2 = receiver.recv().await.unwrap();
    let event3 = receiver.recv().await.unwrap();

    assert_eq!(event1.agent_id().map(|a| a.as_str()), Some("agent-1"));
    assert_eq!(event2.agent_id().map(|a| a.as_str()), Some("agent-2"));
    assert_eq!(event3.agent_id().map(|a| a.as_str()), Some("agent-3"));
}

#[tokio::test]
async fn test_event_bus_broadcast_to_multiple_subscribers() {
    let bus = AgentEventBus::new(10);
    let mut receiver1 = bus.subscribe();
    let mut receiver2 = bus.subscribe();

    let event = AgentEvent::AgentStarted {
        agent_id: AgentId::new("broadcast-test"),
        pid: 1234,
        port: 8080,
    };

    bus.publish(event);

    let received1 = receiver1.recv().await.unwrap();
    let received2 = receiver2.recv().await.unwrap();

    assert_eq!(received1.agent_id().map(|a| a.as_str()), Some("broadcast-test"));
    assert_eq!(received2.agent_id().map(|a| a.as_str()), Some("broadcast-test"));
}

#[tokio::test]
async fn test_event_bus_different_event_types() {
    let bus = AgentEventBus::new(10);
    let mut receiver = bus.subscribe();

    bus.publish(AgentEvent::AgentStartRequested {
        agent_id: AgentId::new("a1"),
    });
    bus.publish(AgentEvent::AgentStarted {
        agent_id: AgentId::new("a1"),
        pid: 100,
        port: 8080,
    });
    bus.publish(AgentEvent::AgentStopped {
        agent_id: AgentId::new("a1"),
        exit_code: Some(0),
    });

    let e1 = receiver.recv().await.unwrap();
    let e2 = receiver.recv().await.unwrap();
    let e3 = receiver.recv().await.unwrap();

    assert_eq!(e1.event_type(), "agent_start_requested");
    assert_eq!(e2.event_type(), "agent_started");
    assert_eq!(e3.event_type(), "agent_stopped");
}

#[test]
fn test_event_bus_debug_format() {
    let bus = AgentEventBus::new(50);
    let debug_str = format!("{:?}", bus);

    assert!(debug_str.contains("AgentEventBus"));
    assert!(debug_str.contains("sender"));
}
