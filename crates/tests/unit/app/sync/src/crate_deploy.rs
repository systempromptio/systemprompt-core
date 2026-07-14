//! Tests for `CrateDeployService` construction and Debug rendering. The
//! full deploy flow is exercised through the `CommandRunner` seam in
//! `crate_deploy_flow`.

use systemprompt_identifiers::TenantId;
use systemprompt_sync::crate_deploy::CrateDeployService;
use systemprompt_sync::{SyncApiClient, SyncConfig};

fn sample_config() -> SyncConfig {
    SyncConfig::builder(
        TenantId::new("acme"),
        "https://api.example.com",
        "deploy-token",
        "/services",
    )
    .build()
}

#[test]
fn new_constructs_service_with_config_and_client() {
    let cfg = sample_config();
    let client = SyncApiClient::new("https://api.example.com", "deploy-token").expect("client");
    let service = CrateDeployService::new(cfg, client);
    let dbg = format!("{service:?}");
    assert!(dbg.contains("CrateDeployService"));
}

#[test]
fn debug_includes_config_and_client_fields() {
    let cfg = sample_config();
    let client = SyncApiClient::new("https://api.example.com", "deploy-token").expect("client");
    let service = CrateDeployService::new(cfg, client);
    let dbg = format!("{service:?}");
    assert!(dbg.contains("acme"));
    assert!(dbg.contains("api.example.com"));
}
