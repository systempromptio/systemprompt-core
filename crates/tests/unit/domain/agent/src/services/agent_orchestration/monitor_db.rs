// DB-backed tests for AgentMonitor and
// AgentDatabaseService::get_unresponsive_agents over an injected registry,
// using a bound TcpListener as a live healthy port, a closed port as an
// unhealthy one, and wiremock as an A2A card endpoint.

use std::collections::HashMap;

use systemprompt_agent::repository::agent_service::AgentServiceRepository;
use systemprompt_agent::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_agent::services::agent_orchestration::monitor::AgentMonitor;
use systemprompt_agent::services::registry::AgentRegistry;
use systemprompt_models::ServicesConfig;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::super::a2a_server::a2a_helpers::agent_config;
use crate::repository::try_pool;

const DEAD_PID: u32 = 4_000_000_002;

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
    let repo = AgentServiceRepository::new(pool).expect("repo");
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
    let Some(pool) = try_pool().await else {
        return;
    };
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
    let Some(pool) = try_pool().await else {
        return;
    };
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
    let Some(pool) = try_pool().await else {
        return;
    };
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

#[tokio::test]
async fn monitor_all_agents_buckets_healthy_and_failed() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (listener, port) = free_port_listener().await;
    let accept_loop = tokio::spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });

    let healthy = unique_name("mon_all_h");
    let dead = unique_name("mon_all_d");
    let svc = db_service_with(&pool, &[(&healthy, port), (&dead, 9421)]);
    svc.register_agent(&healthy, std::process::id(), port)
        .await
        .expect("register healthy");
    svc.register_agent(&dead, DEAD_PID, 9421)
        .await
        .expect("register dead");

    let monitor =
        AgentMonitor::with_db_service(db_service_with(&pool, &[(&healthy, port), (&dead, 9421)]));
    let report = monitor.monitor_all_agents().await.expect("monitor all");
    assert!(report.healthy.iter().any(|agent| agent == &healthy));
    assert!(report.failed.iter().any(|agent| agent == &dead));
    assert_eq!(report.total_agents(), 2);

    accept_loop.abort();
    svc.remove_agent_service(&healthy).await.ok();
    svc.remove_agent_service(&dead).await.ok();
}

#[tokio::test]
async fn unresponsive_agents_include_running_agent_without_card_endpoint() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let (listener, port) = free_port_listener().await;
    drop(listener);

    let name = unique_name("mon_unresp");
    let svc = db_service_with(&pool, &[(&name, port)]);
    svc.register_agent(&name, std::process::id(), port)
        .await
        .expect("register");

    let unresponsive = svc.get_unresponsive_agents().await.expect("unresponsive");
    let entry = unresponsive
        .iter()
        .find(|(agent, _)| agent == &name)
        .expect("agent listed unresponsive");
    assert_eq!(entry.1, Some(std::process::id()));

    svc.remove_agent_service(&name).await.ok();
}

#[tokio::test]
async fn unresponsive_agents_skip_agent_serving_valid_card() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/agent-card.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "mock-agent",
            "url": "http://localhost/a2a"
        })))
        .mount(&server)
        .await;
    let port = server.address().port();

    let name = unique_name("mon_resp");
    let svc = db_service_with(&pool, &[(&name, port)]);
    svc.register_agent(&name, std::process::id(), port)
        .await
        .expect("register");

    let unresponsive = svc.get_unresponsive_agents().await.expect("unresponsive");
    assert!(!unresponsive.iter().any(|(agent, _)| agent == &name));

    svc.remove_agent_service(&name).await.ok();
}
