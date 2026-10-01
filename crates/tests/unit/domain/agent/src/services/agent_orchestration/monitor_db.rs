// DB-backed tests for AgentMonitor over an injected registry, using a bound
// TcpListener as a live healthy port and a closed port as an unhealthy one.

use std::collections::HashMap;

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_agent::services::agent_orchestration::monitor::AgentMonitor;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_models::ServicesConfig;
use uuid::Uuid;

use super::super::a2a_server::a2a_helpers::agent_config;
use systemprompt_test_fixtures::test_db_pool;

fn unique_name(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn db_service_with(
    pool: &systemprompt_database::DbPool,
    names_and_ports: &[(&str, u16)],
) -> AgentDatabaseService {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let mut agents = HashMap::new();
    for (name, port) in names_and_ports {
        let mut config = agent_config(name);
        config.port = *port;
        agents.insert((*name).to_owned(), config);
    }
    let registry = AgentRegistry::from_config(ServicesConfig {
        agents,
        ..ServicesConfig::default()
    });
    let repo = AgentServiceRepository::new(
        pool,
        systemprompt_identifiers::InstanceId::new("test-instance"),
    );
    AgentDatabaseService::with_registry(repo, registry)
}

async fn free_port_listener() -> (tokio::net::TcpListener, u16) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    (listener, port)
}

#[tokio::test]
async fn health_check_reports_not_running_for_unknown_agent() {
    let pool = test_db_pool().await;
    let name = unique_name("mon_missing");
    let monitor = AgentMonitor::with_db_service(db_service_with(&pool, &[(&name, 9420)]));

    let result = monitor
        .comprehensive_health_check(&name)
        .await
        .expect("health check");
    assert!(!result.healthy);
    assert!(result.message.contains("not in running state"));
    assert_eq!(result.response_time_ms, 0);
}

#[tokio::test]
async fn health_check_passes_for_live_process_with_open_port() {
    let pool = test_db_pool().await;
    let (listener, port) = free_port_listener().await;
    let accept_loop = tokio::spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });

    let name = unique_name("mon_live");
    let svc = db_service_with(&pool, &[(&name, port)]);
    svc.register_agent(&name, std::process::id(), port)
        .await
        .expect("register");

    let monitor = AgentMonitor::with_db_service(db_service_with(&pool, &[(&name, port)]));
    let result = monitor
        .comprehensive_health_check(&name)
        .await
        .expect("health check");
    assert!(result.healthy);
    assert!(result.message.contains("TCP connection successful"));

    accept_loop.abort();
    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn health_check_fails_for_live_process_with_closed_port() {
    let pool = test_db_pool().await;
    let (listener, port) = free_port_listener().await;
    drop(listener);

    let name = unique_name("mon_closed");
    let svc = db_service_with(&pool, &[(&name, port)]);
    svc.register_agent(&name, std::process::id(), port)
        .await
        .expect("register");

    let monitor = AgentMonitor::with_db_service(db_service_with(&pool, &[(&name, port)]));
    let result = monitor
        .comprehensive_health_check(&name)
        .await
        .expect("health check");
    assert!(!result.healthy);
    assert!(result.message.contains("Connection failed") || result.message.contains("timeout"));

    svc.remove_agent_service(&name).await.ok();
}

