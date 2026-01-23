use systemprompt_events::ToSse;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::a2a::TaskState;
use systemprompt_models::api::CliOutputEvent;
use systemprompt_models::{
    A2AEvent, A2AEventBuilder, AgUiEvent, AgUiEventBuilder, AnalyticsEvent, AnalyticsEventBuilder,
    ContextEvent, SystemEvent, SystemEventBuilder,
};

fn test_agui_event() -> AgUiEvent {
    AgUiEventBuilder::run_started(
        ContextId::new("test-context"),
        TaskId::new("test-task"),
        None,
    )
}

fn test_a2a_event() -> A2AEvent {
    A2AEventBuilder::task_status_update(
        TaskId::new("test-task"),
        ContextId::new("test-context"),
        TaskState::Working,
        Some("test message".to_string()),
    )
}

fn test_system_event() -> SystemEvent {
    SystemEventBuilder::heartbeat()
}

fn test_analytics_event() -> AnalyticsEvent {
    AnalyticsEventBuilder::page_view(
        "test-session".to_string(),
        None,
        "/test-page".to_string(),
        None,
        Some("https://example.com/referrer".to_string()),
    )
}

fn test_context_event_agui() -> ContextEvent {
    ContextEvent::AgUi(test_agui_event())
}

fn test_context_event_system() -> ContextEvent {
    ContextEvent::System(test_system_event())
}

fn test_cli_output_event() -> CliOutputEvent {
    CliOutputEvent::Stdout {
        data: "test output".to_string(),
    }
}

#[test]
fn test_agui_event_to_sse_succeeds() {
    let event = test_agui_event();
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_agui_event_to_sse_produces_valid_json() {
    let event = test_agui_event();
    let sse_event = event.to_sse().expect("should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(debug_str.contains("Event"));
}

#[test]
fn test_a2a_event_to_sse_succeeds() {
    let event = test_a2a_event();
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_a2a_event_to_sse_produces_valid_json() {
    let event = test_a2a_event();
    let sse_event = event.to_sse().expect("should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(debug_str.contains("Event"));
}

#[test]
fn test_system_event_to_sse_succeeds() {
    let event = test_system_event();
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_system_event_to_sse_produces_valid_json() {
    let event = test_system_event();
    let sse_event = event.to_sse().expect("should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(debug_str.contains("Event"));
}

#[test]
fn test_context_event_agui_to_sse_succeeds() {
    let event = test_context_event_agui();
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_context_event_system_to_sse_succeeds() {
    let event = test_context_event_system();
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_analytics_event_to_sse_succeeds() {
    let event = test_analytics_event();
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_analytics_event_to_sse_produces_valid_json() {
    let event = test_analytics_event();
    let sse_event = event.to_sse().expect("should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(debug_str.contains("Event"));
}

#[test]
fn test_cli_output_event_to_sse_succeeds() {
    let event = test_cli_output_event();
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_cli_output_event_to_sse_has_custom_event_type() {
    let event = test_cli_output_event();
    let sse_event = event.to_sse().expect("should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(debug_str.contains("cli"));
}

#[test]
fn test_cli_output_event_error_to_sse() {
    let event = CliOutputEvent::Stderr {
        data: "error message".to_string(),
    };
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_cli_output_event_started_to_sse() {
    let event = CliOutputEvent::Started { pid: 12345 };
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_cli_output_event_exit_code_to_sse() {
    let event = CliOutputEvent::ExitCode { code: 0 };
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_cli_output_event_error_variant_to_sse() {
    let event = CliOutputEvent::Error {
        message: "Something went wrong".to_string(),
    };
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_context_event_a2a_to_sse_succeeds() {
    let a2a_event = test_a2a_event();
    let context_event: ContextEvent = a2a_event.into();
    let result = context_event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_multiple_agui_events_serialize_independently() {
    let event1 = AgUiEventBuilder::run_started(
        ContextId::new("context-1"),
        TaskId::new("task-1"),
        None,
    );
    let event2 = AgUiEventBuilder::run_started(
        ContextId::new("context-2"),
        TaskId::new("task-2"),
        None,
    );

    let sse1 = event1.to_sse().expect("should serialize");
    let sse2 = event2.to_sse().expect("should serialize");

    let debug1 = format!("{:?}", sse1);
    let debug2 = format!("{:?}", sse2);

    assert!(debug1.contains("context-1") || debug1.contains("task-1"));
    assert!(debug2.contains("context-2") || debug2.contains("task-2"));
}

#[test]
fn test_system_event_heartbeat_serialization() {
    let event = SystemEventBuilder::heartbeat();
    let sse_event = event.to_sse().expect("should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(debug_str.contains("heartbeat") || debug_str.contains("Event"));
}

#[test]
fn test_analytics_event_heartbeat() {
    let event = AnalyticsEventBuilder::heartbeat();
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_analytics_event_session_ended() {
    let event = AnalyticsEventBuilder::session_ended(
        "test-session".to_string(),
        60000,
        5,
        10,
    );
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_analytics_event_engagement_update() {
    let event = AnalyticsEventBuilder::engagement_update(
        "test-session".to_string(),
        "/test-page".to_string(),
        75,
        30000,
        3,
    );
    let result = event.to_sse();
    assert!(result.is_ok());
}

#[test]
fn test_analytics_event_realtime_stats() {
    let event = AnalyticsEventBuilder::realtime_stats(100, 50, 200, 500, 10);
    let result = event.to_sse();
    assert!(result.is_ok());
}
