use std::time::Duration;
use systemprompt_cli::presentation::StartupRenderer;
use systemprompt_traits::{
    Phase, ServiceInfo, ServiceState, ServiceType, StartupEvent, startup_channel,
};

fn svc(name: &str, ty: ServiceType, state: ServiceState) -> ServiceInfo {
    ServiceInfo {
        name: name.to_owned(),
        service_type: ty,
        port: Some(8080),
        state,
        startup_time: Some(Duration::from_millis(120)),
    }
}

#[tokio::test]
async fn renderer_drives_full_happy_path() {
    let (tx, rx) = startup_channel();
    let renderer = StartupRenderer::new(rx);

    tx.unbounded_send(StartupEvent::PhaseStarted {
        phase: Phase::McpServers,
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::McpServerReady {
        name: "mcp-a".to_owned(),
        port: 9001,
        startup_time: Duration::from_millis(50),
        tools: 3,
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::McpServerFailed {
        name: "mcp-b".to_owned(),
        error: "boom".to_owned(),
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::McpReconciliationComplete {
        running: 1,
        required: 2,
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::PhaseCompleted {
        phase: Phase::McpServers,
    })
    .unwrap();

    tx.unbounded_send(StartupEvent::PhaseStarted {
        phase: Phase::Agents,
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::AgentReady {
        name: "agent-a".to_owned(),
        port: 9100,
        startup_time: Duration::from_millis(80),
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::AgentFailed {
        name: "agent-b".to_owned(),
        error: "fail".to_owned(),
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::AgentReconciliationComplete {
        running: 1,
        total: 2,
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::PhaseCompleted {
        phase: Phase::Agents,
    })
    .unwrap();

    tx.unbounded_send(StartupEvent::PortConflict {
        port: 8080,
        pid: 12345,
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::SchedulerInitializing)
        .unwrap();
    tx.unbounded_send(StartupEvent::SchedulerReady { job_count: 4 })
        .unwrap();
    tx.unbounded_send(StartupEvent::Warning {
        message: "warm".to_owned(),
        context: Some("ctx".to_owned()),
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::Warning {
        message: "no-ctx".to_owned(),
        context: None,
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::Error {
        message: "non-fatal".to_owned(),
        fatal: false,
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::Error {
        message: "fatal".to_owned(),
        fatal: true,
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::StartupComplete {
        duration: Duration::from_secs(2),
        api_url: "http://localhost:8080".to_owned(),
        services: vec![
            svc("mcp-a", ServiceType::Mcp, ServiceState::Running),
            svc("new-extra", ServiceType::Agent, ServiceState::Starting),
        ],
    })
    .unwrap();
    drop(tx);

    renderer.run().await;
}

#[tokio::test]
async fn renderer_handles_phase_failed_branch() {
    let (tx, rx) = startup_channel();
    let renderer = StartupRenderer::new(rx);

    tx.unbounded_send(StartupEvent::PhaseStarted {
        phase: Phase::McpServers,
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::PhaseFailed {
        phase: Phase::McpServers,
        error: "init failed".to_owned(),
    })
    .unwrap();
    // PhaseFailed for a phase with no spinner exercises the warning branch.
    tx.unbounded_send(StartupEvent::PhaseFailed {
        phase: Phase::Agents,
        error: "no-spinner".to_owned(),
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::StartupFailed {
        error: "bad".to_owned(),
        duration: Duration::from_secs(1),
    })
    .unwrap();
    drop(tx);

    renderer.run().await;
}

#[tokio::test]
async fn renderer_ignores_irrelevant_events_and_completes() {
    let (tx, rx) = startup_channel();
    let renderer = StartupRenderer::new(rx);

    // Pass through every "fall-through" event type to drive the `_ => false`
    // arm in `handle_terminal_event`.
    tx.unbounded_send(StartupEvent::PortCheckStarted { port: 7000 })
        .unwrap();
    tx.unbounded_send(StartupEvent::PortAvailable { port: 7000 })
        .unwrap();
    tx.unbounded_send(StartupEvent::PortConflictResolved { port: 7000 })
        .unwrap();
    tx.unbounded_send(StartupEvent::MigrationStarted).unwrap();
    tx.unbounded_send(StartupEvent::MigrationApplied {
        name: "001".to_owned(),
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::MigrationComplete {
        applied: 1,
        skipped: 0,
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::DatabaseValidated).unwrap();
    tx.unbounded_send(StartupEvent::RoutesConfiguring).unwrap();
    tx.unbounded_send(StartupEvent::RoutesConfigured { module_count: 5 })
        .unwrap();
    tx.unbounded_send(StartupEvent::Info {
        message: "hi".to_owned(),
    })
    .unwrap();
    tx.unbounded_send(StartupEvent::StartupComplete {
        duration: Duration::from_millis(10),
        api_url: "http://x".to_owned(),
        services: vec![],
    })
    .unwrap();
    drop(tx);

    renderer.run().await;
}
