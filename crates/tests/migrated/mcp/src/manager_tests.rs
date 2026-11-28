//! The manager's lifecycle: concurrent reconciles, restarts and shutdown on
//! one set of servers.

use std::collections::BTreeMap;
use std::time::Duration;

use systemprompt_engine::mcp::{
    ClientInfo, ManagerConfig, McpManager, ServerConfig, ServerSpec, ServerStatus,
};

fn unstartable(name: &str) -> ServerConfig {
    ServerConfig::new(ServerSpec::Local {
        command: format!("definitely-not-a-command-{name}"),
        args: Vec::new(),
        env: BTreeMap::new(),
        cwd: None,
    })
}

#[tokio::test]
async fn concurrent_lifecycle_operations_settle_to_one_consistent_state() {
    let manager = McpManager::new(ManagerConfig::new(ClientInfo::new(
        "migrated-mcp-tests",
        "0",
    )))
    .unwrap();
    let mut tasks = Vec::new();
    for round in 0..8 {
        let manager = manager.clone();
        tasks.push(tokio::spawn(async move {
            let mut servers = BTreeMap::from([("a".to_owned(), unstartable("a"))]);
            if round % 2 == 0 {
                servers.insert("b".to_owned(), unstartable("b"));
            }
            manager.reconcile(servers);
            drop(manager.restart("a"));
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    manager.reconcile(BTreeMap::from([("a".to_owned(), unstartable("a"))]));
    tokio::time::timeout(Duration::from_secs(10), manager.settle())
        .await
        .unwrap();
    let status = manager.status();
    assert_eq!(status.keys().collect::<Vec<_>>(), ["a"]);
    assert!(
        matches!(status["a"], ServerStatus::Failed { .. }),
        "{status:?}"
    );
    tokio::time::timeout(Duration::from_secs(10), manager.shutdown())
        .await
        .unwrap();
    assert!(manager.status().is_empty());
}
