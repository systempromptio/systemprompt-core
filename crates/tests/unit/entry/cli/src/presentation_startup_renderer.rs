//! Tests for the startup renderer.
//!
//! `StartupRenderer::run` drains a startup-event channel and drives the
//! banner, spinner state, service table, warning, and completion widgets.
//! Nothing called it, so the whole `presentation` render path was dark.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::time::Duration;

use systemprompt_cli::presentation::StartupRenderer;
use systemprompt_traits::{
    Phase, ServiceInfo, ServiceState, ServiceType, StartupEvent, startup_channel,
};

fn service(name: &str, service_type: ServiceType, state: ServiceState) -> ServiceInfo {
    ServiceInfo {
        name: name.to_owned(),
        service_type,
        port: Some(9200),
        state,
        startup_time: Some(Duration::from_millis(120)),
    }
}

async fn drive(events: Vec<StartupEvent>) {
    let (tx, rx) = startup_channel();
    let renderer = StartupRenderer::new(rx);
    let handle = tokio::spawn(renderer.run());

    for event in events {
        tx.unbounded_send(event).expect("renderer is receiving");
    }
    drop(tx);

    handle.await.expect("renderer task completes");
}

#[tokio::test]
async fn a_full_successful_startup_sequence_is_rendered_and_terminates() {
    drive(vec![
        StartupEvent::PhaseStarted {
            phase: Phase::PreFlight,
        },
        StartupEvent::Info {
            message: "Running in local-only mode".to_owned(),
        },
        StartupEvent::PhaseCompleted {
            phase: Phase::PreFlight,
        },
        StartupEvent::PhaseStarted {
            phase: Phase::Database,
        },
        StartupEvent::MigrationStarted,
        StartupEvent::MigrationApplied {
            name: "001_init".to_owned(),
        },
        StartupEvent::MigrationComplete {
            applied: 1,
            skipped: 4,
        },
        StartupEvent::DatabaseValidated,
        StartupEvent::PhaseCompleted {
            phase: Phase::Database,
        },
        StartupEvent::PhaseStarted {
            phase: Phase::McpServers,
        },
        StartupEvent::McpServerStarting {
            name: "covmcp".to_owned(),
            port: 5100,
        },
        StartupEvent::McpServerHealthCheck {
            name: "covmcp".to_owned(),
            attempt: 1,
            max_attempts: 3,
        },
        StartupEvent::McpServerReady {
            name: "covmcp".to_owned(),
            port: 5100,
            startup_time: Duration::from_millis(80),
            tools: Some(4),
        },
        StartupEvent::McpReconciliationComplete {
            running: 1,
            required: 1,
        },
        StartupEvent::PhaseCompleted {
            phase: Phase::McpServers,
        },
        StartupEvent::PhaseStarted {
            phase: Phase::Agents,
        },
        StartupEvent::AgentStarting {
            name: "covagent".to_owned(),
            port: 9200,
        },
        StartupEvent::AgentReady {
            name: "covagent".to_owned(),
            port: 9200,
            startup_time: Duration::from_millis(90),
        },
        StartupEvent::AgentReconciliationComplete {
            running: 1,
            total: 1,
        },
        StartupEvent::PhaseCompleted {
            phase: Phase::Agents,
        },
        StartupEvent::RoutesConfiguring,
        StartupEvent::RoutesConfigured { module_count: 7 },
        StartupEvent::ExtensionRouteMounted {
            name: "content".to_owned(),
            path: "/api/v1/content".to_owned(),
            auth_required: true,
        },
        StartupEvent::SchedulerInitializing,
        StartupEvent::SchedulerJobRegistered {
            name: "cleanup".to_owned(),
            schedule: "0 * * * *".to_owned(),
        },
        StartupEvent::SchedulerReady {
            scheduled: 1,
            available: 9,
        },
        StartupEvent::ServerBinding {
            address: "127.0.0.1:8080".to_owned(),
        },
        StartupEvent::ServerListening {
            address: "127.0.0.1:8080".to_owned(),
            pid: 4242,
        },
        StartupEvent::StartupComplete {
            duration: Duration::from_secs(3),
            api_url: "http://127.0.0.1:8080".to_owned(),
            services: vec![
                service("covmcp", ServiceType::Mcp, ServiceState::Running),
                service("covagent", ServiceType::Agent, ServiceState::Running),
            ],
        },
    ])
    .await;
}

#[tokio::test]
async fn a_failed_startup_sequence_renders_its_failures_and_terminates() {
    drive(vec![
        StartupEvent::PhaseStarted {
            phase: Phase::Database,
        },
        StartupEvent::PortCheckStarted { port: 8080 },
        StartupEvent::PortConflict {
            port: 8080,
            pid: 999_999,
        },
        StartupEvent::PortConflictResolved { port: 8080 },
        StartupEvent::PortAvailable { port: 8080 },
        StartupEvent::Warning {
            message: "config drift detected".to_owned(),
            context: Some("profile".to_owned()),
        },
        StartupEvent::McpServerFailed {
            name: "covmcp".to_owned(),
            error: "binary missing".to_owned(),
        },
        StartupEvent::McpServiceCleanup {
            name: "covmcp".to_owned(),
            reason: "not in manifest".to_owned(),
        },
        StartupEvent::AgentFailed {
            name: "covagent".to_owned(),
            error: "port in use".to_owned(),
        },
        StartupEvent::AgentCleanup {
            name: "covagent".to_owned(),
            reason: "disabled".to_owned(),
        },
        StartupEvent::BootstrapJobStarted {
            name: "seed".to_owned(),
        },
        StartupEvent::BootstrapJobCompleted {
            name: "seed".to_owned(),
            success: false,
            message: Some("skipped".to_owned()),
        },
        StartupEvent::PhaseFailed {
            phase: Phase::Database,
            error: "connection refused".to_owned(),
        },
        StartupEvent::Error {
            message: "fatal".to_owned(),
            fatal: true,
        },
        StartupEvent::StartupFailed {
            error: "database unreachable".to_owned(),
            duration: Duration::from_secs(1),
        },
    ])
    .await;
}

#[tokio::test]
async fn a_dropped_sender_ends_the_renderer_without_a_terminal_event() {
    drive(vec![StartupEvent::Info {
        message: "nothing else follows".to_owned(),
    }])
    .await;
}

#[tokio::test]
async fn events_after_a_terminal_event_do_not_keep_the_renderer_alive() {
    let (tx, rx) = startup_channel();
    let renderer = StartupRenderer::new(rx);
    let handle = tokio::spawn(renderer.run());

    tx.unbounded_send(StartupEvent::StartupFailed {
        error: "stopped early".to_owned(),
        duration: Duration::from_millis(5),
    })
    .unwrap();
    let _ = tx.unbounded_send(StartupEvent::Info {
        message: "ignored".to_owned(),
    });
    drop(tx);

    handle.await.expect("renderer stops on the terminal event");
}
