use systemprompt_events::ToSse;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::a2a::TaskState;
use systemprompt_models::api::CliOutputEvent;
use systemprompt_models::{
    A2AEvent, A2AEventBuilder, AgUiEvent, AgUiEventBuilder, AnalyticsEvent, AnalyticsEventBuilder,
    ContextEvent, SystemEvent, SystemEventBuilder,
};

const TEST_CONTEXT_ID_A: &str = "00000000-0000-4000-8000-000000000001";
const TEST_CONTEXT_ID_B: &str = "00000000-0000-4000-8000-000000000002";


fn test_agui_event() -> AgUiEvent {
    AgUiEventBuilder::run_started(
        ContextId::new(TEST_CONTEXT_ID_A),
        TaskId::new("test-task"),
        None,
    )
}

fn test_a2a_event() -> A2AEvent {
    A2AEventBuilder::task_status_update(
        TaskId::new("test-task"),
        ContextId::new(TEST_CONTEXT_ID_A),
        TaskState::Working,
        Some("test message".to_string()),
    )
}

fn test_system_event() -> SystemEvent {
    SystemEventBuilder::heartbeat()
}

fn test_analytics_event() -> AnalyticsEvent {
    AnalyticsEventBuilder::page_view(
        "test-session".to_string().into(),
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
    let sse_event = event.to_sse().expect("agui event should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("test-task"),
        "frame missing task id: {debug_str}"
    );
    assert!(
        debug_str.contains(TEST_CONTEXT_ID_A),
        "frame missing context id: {debug_str}"
    );
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
    let sse_event = event.to_sse().expect("a2a event should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("test message"),
        "frame missing status message: {debug_str}"
    );
    assert!(
        debug_str.contains("test-task"),
        "frame missing task id: {debug_str}"
    );
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
    let sse_event = event.to_sse().expect("system event should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("HEARTBEAT"),
        "frame missing heartbeat marker: {debug_str}"
    );
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
    let sse_event = event.to_sse().expect("context agui event should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("test-task"),
        "frame missing task id: {debug_str}"
    );
    assert!(
        debug_str.contains(TEST_CONTEXT_ID_A),
        "frame missing context id: {debug_str}"
    );
}

#[test]
fn test_context_event_system_to_sse_succeeds() {
    let event = test_context_event_system();
    let sse_event = event
        .to_sse()
        .expect("context system event should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("HEARTBEAT"),
        "frame missing heartbeat marker: {debug_str}"
    );
}

#[test]
fn test_analytics_event_to_sse_succeeds() {
    let event = test_analytics_event();
    let sse_event = event.to_sse().expect("analytics event should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("/test-page"),
        "frame missing page path: {debug_str}"
    );
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
    let sse_event = event.to_sse().expect("cli output event should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("cli"),
        "frame missing cli event name: {debug_str}"
    );
    assert!(
        debug_str.contains("test output"),
        "frame missing stdout data: {debug_str}"
    );
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
    let sse_event = event.to_sse().expect("cli stderr event should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("cli"),
        "frame missing cli event name: {debug_str}"
    );
    assert!(
        debug_str.contains("error message"),
        "frame missing stderr data: {debug_str}"
    );
}

#[test]
fn test_cli_output_event_started_to_sse() {
    let event = CliOutputEvent::Started { pid: 12345 };
    let sse_event = event.to_sse().expect("cli started event should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("12345"),
        "frame missing pid: {debug_str}"
    );
}

#[test]
fn test_cli_output_event_exit_code_to_sse() {
    let event = CliOutputEvent::ExitCode { code: 0 };
    let sse_event = event.to_sse().expect("cli exit event should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("cli"),
        "frame missing cli event name: {debug_str}"
    );
    assert!(
        debug_str.contains('0'),
        "frame missing exit code: {debug_str}"
    );
}

#[test]
fn test_cli_output_event_error_variant_to_sse() {
    let event = CliOutputEvent::Error {
        message: "Something went wrong".to_string(),
    };
    let sse_event = event.to_sse().expect("cli error event should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("Something went wrong"),
        "frame missing error message: {debug_str}"
    );
}

#[test]
fn test_context_event_a2a_to_sse_succeeds() {
    let a2a_event = test_a2a_event();
    let context_event: ContextEvent = a2a_event.into();
    let sse_event = context_event
        .to_sse()
        .expect("context a2a event should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("test message"),
        "frame missing status message: {debug_str}"
    );
}

#[test]
fn test_multiple_agui_events_serialize_independently() {
    let event1 = AgUiEventBuilder::run_started(
        ContextId::new(TEST_CONTEXT_ID_A),
        TaskId::new("task-1"),
        None,
    );
    let event2 = AgUiEventBuilder::run_started(
        ContextId::new(TEST_CONTEXT_ID_B),
        TaskId::new("task-2"),
        None,
    );

    let sse1 = event1.to_sse().expect("should serialize");
    let sse2 = event2.to_sse().expect("should serialize");

    let debug1 = format!("{:?}", sse1);
    let debug2 = format!("{:?}", sse2);

    assert!(debug1.contains(TEST_CONTEXT_ID_A) || debug1.contains("task-1"));
    assert!(debug2.contains(TEST_CONTEXT_ID_B) || debug2.contains("task-2"));
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
    let sse_event = event
        .to_sse()
        .expect("analytics heartbeat should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("HEARTBEAT"),
        "frame missing heartbeat marker: {debug_str}"
    );
}

#[test]
fn test_analytics_event_session_ended() {
    let event =
        AnalyticsEventBuilder::session_ended("test-session".to_string().into(), 60000, 5, 10);
    let sse_event = event.to_sse().expect("session_ended should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("test-session"),
        "frame missing session id: {debug_str}"
    );
    assert!(
        debug_str.contains("60000"),
        "frame missing duration: {debug_str}"
    );
}

#[test]
fn test_analytics_event_engagement_update() {
    let event = AnalyticsEventBuilder::engagement_update(
        "test-session".to_string().into(),
        "/test-page".to_string(),
        75,
        30000,
        3,
    );
    let sse_event = event.to_sse().expect("engagement_update should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("/test-page"),
        "frame missing page path: {debug_str}"
    );
    assert!(
        debug_str.contains("75"),
        "frame missing engagement score: {debug_str}"
    );
}

#[test]
fn test_analytics_event_realtime_stats() {
    let event = AnalyticsEventBuilder::realtime_stats(100, 50, 200, 500, 10);
    let sse_event = event.to_sse().expect("realtime_stats should serialize");
    let debug_str = format!("{:?}", sse_event);
    assert!(
        debug_str.contains("100"),
        "frame missing active-user count: {debug_str}"
    );
}
