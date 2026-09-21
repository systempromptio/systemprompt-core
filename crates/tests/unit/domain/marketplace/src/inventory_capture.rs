use super::*;

#[tokio::test]
async fn general_rule_capture_is_idempotent_and_preserves_membership_intervals() {
    let f = Fixture::new().await;
    std::fs::create_dir(f.root.path().join("rules")).expect("rules");
    std::fs::write(f.root.path().join("rules/style.md"), "# Style rules\n").expect("rule");
    f.refresh().await;
    let id = configured_identity(&f.owner, "rule", "style");
    let request = BaselinePreparation {
        operation_id: TaskId::generate(),
        after: None,
        limit: 100,
    };
    let service = InventoryService::new(f.repository.clone());
    let first = service
        .prepare_baselines(
            &BaselineScope {
                owner: &f.owner,
                actor: &f.owner,
                root: f.root.path(),
                services: &ServicesConfig::default(),
            },
            &request,
        )
        .await
        .expect("capture")
        .remove(0);
    assert_eq!(first.status, "ready");
    let revision = first.revision_id.expect("revision");
    let files = f
        .repository
        .get_revision_files(&f.owner, &revision)
        .await
        .expect("files");
    assert_eq!(files.0["style.md"].bytes, b"# Style rules\n");
    let before = f
        .repository
        .inventory_membership(&f.owner, &id, chrono::Utc::now())
        .await
        .expect("membership");
    let repeated = service
        .prepare_baselines(
            &BaselineScope {
                owner: &f.owner,
                actor: &f.owner,
                root: f.root.path(),
                services: &ServicesConfig::default(),
            },
            &request,
        )
        .await
        .expect("retry")
        .remove(0);
    assert_eq!(repeated.revision_id.as_ref(), Some(&revision));
    let after = f
        .repository
        .inventory_membership(&f.owner, &id, chrono::Utc::now())
        .await
        .expect("membership");
    let (
        ObservedMembership::Known {
            effective_from: a, ..
        },
        ObservedMembership::Known {
            effective_from: b, ..
        },
    ) = (before, after)
    else {
        panic!("known membership")
    };
    assert_eq!(a, b);
    assert!(f.repository.inventory(&f.owner, None, 101).await.is_err());
}

#[tokio::test]
async fn failed_complete_scan_preserves_membership_and_records_health() {
    let f = Fixture::new().await;
    f.skill("retained", true);
    f.refresh().await;
    let initial = f
        .repository
        .inventory_status(&f.owner)
        .await
        .expect("status");
    let result = InventoryService::new(f.repository.clone())
        .refresh(
            &f.owner,
            &f.root.path().join("missing"),
            &ServicesConfig::default(),
        )
        .await;
    assert!(result.is_err());
    let status = f
        .repository
        .inventory_status(&f.owner)
        .await
        .expect("status");
    assert_eq!(status.generation, initial.generation);
    assert!(status.last_error.is_some());
    assert_eq!(
        f.repository
            .inventory(&f.owner, None, 100)
            .await
            .expect("retained")
            .len(),
        1
    );
}

#[tokio::test]
async fn bounded_pages_and_immutable_bindings_do_not_adopt_same_named_resources() {
    let f = Fixture::new().await;
    f.skill("local", true);
    let (first, _) = f.imported("first").await;
    let (second, _) = f.imported("second").await;
    f.refresh().await;
    let page = f
        .repository
        .inventory(&f.owner, None, 1)
        .await
        .expect("first page");
    let next = f
        .repository
        .inventory(&f.owner, Some(&page[0].entry_id), 1)
        .await
        .expect("second page");
    assert_ne!(page[0].entry_id, next[0].entry_id);
    let id = configured_identity(&f.owner, "skill", "local");
    f.repository
        .bind_inventory_resource(&f.owner, &f.owner, &id, &first)
        .await
        .expect("bind");
    f.repository
        .bind_inventory_resource(&f.owner, &f.owner, &id, &first)
        .await
        .expect("identical retry");
    assert!(
        f.repository
            .bind_inventory_resource(&f.owner, &f.owner, &id, &second)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn inline_agent_and_mcp_baselines_capture_config_without_disk_placeholders() {
    let f = Fixture::new().await;
    let services: ServicesConfig = serde_yaml::from_str(
        r#"
agents:
  captured-agent:
    name: captured-agent
    port: 9123
    endpoint: http://localhost:9123
    enabled: true
    card:
      protocolVersion: "0.2.3"
      displayName: Captured Agent
      description: inline agent
      version: 1.0.0
      preferredTransport: JSONRPC
      capabilities: {streaming: false, pushNotifications: false, stateTransitionHistory: false}
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
      skills: []
      supportsAuthenticatedExtendedCard: false
    metadata: {}
mcp_servers:
  captured-mcp:
    type: external
    endpoint: https://mcp.invalid/rpc
    enabled: true
    display_in_web: true
    oauth: {required: false, scopes: [], audience: mcp, client_id: null}
"#,
    )
    .expect("valid inline services config");
    let service = InventoryService::new(f.repository.clone());
    service
        .refresh(&f.owner, f.root.path(), &services)
        .await
        .expect("refresh configured inventory");
    let outcomes = service
        .prepare_baselines(
            &BaselineScope {
                owner: &f.owner,
                actor: &f.owner,
                root: f.root.path(),
                services: &services,
            },
            &BaselinePreparation {
                operation_id: TaskId::generate(),
                after: None,
                limit: 100,
            },
        )
        .await
        .expect("capture inline configured baselines");
    assert_eq!(outcomes.len(), 2);

    let captured_agent = outcomes
        .iter()
        .find(|outcome| {
            outcome.entry_id == configured_identity(&f.owner, "agent", "captured-agent")
        })
        .and_then(|outcome| outcome.revision_id.as_ref())
        .expect("configured agent revision");
    let agent_files = f
        .repository
        .get_revision_files(&f.owner, captured_agent)
        .await
        .expect("retained inline agent configuration");
    assert_eq!(agent_files.0.len(), 1);
    let captured_agent: systemprompt_models::AgentConfig =
        serde_yaml::from_slice(&agent_files.0["config.yaml"].bytes)
            .expect("deserialize retained agent config");
    assert_eq!(
        serde_json::to_value(captured_agent).expect("agent JSON"),
        serde_json::to_value(&services.agents["captured-agent"]).expect("configured agent JSON")
    );

    let captured_mcp = outcomes
        .iter()
        .find(|outcome| outcome.entry_id == configured_identity(&f.owner, "mcp", "captured-mcp"))
        .and_then(|outcome| outcome.revision_id.as_ref())
        .expect("configured MCP revision");
    let mcp_files = f
        .repository
        .get_revision_files(&f.owner, captured_mcp)
        .await
        .expect("retained inline MCP configuration");
    assert_eq!(mcp_files.0.len(), 1);
    let captured_mcp: systemprompt_models::mcp::Deployment =
        serde_yaml::from_slice(&mcp_files.0["config.yaml"].bytes)
            .expect("deserialize retained MCP config");
    assert_eq!(
        serde_json::to_value(captured_mcp).expect("MCP JSON"),
        serde_json::to_value(&services.mcp_servers["captured-mcp"]).expect("configured MCP JSON")
    );
    assert_eq!(
        std::fs::read_dir(f.root.path())
            .expect("read empty services root")
            .count(),
        0,
        "inline configured resources are captured without filesystem placeholders"
    );
}
