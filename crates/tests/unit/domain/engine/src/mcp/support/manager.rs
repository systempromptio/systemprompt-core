use std::collections::BTreeMap;
use std::time::Duration;

use systemprompt_engine::mcp::{
    ClientInfo, ManagerConfig, McpManager, RemoteHeaders, ServerConfig, ServerSpec, ServerStatus,
};

pub(crate) fn manager() -> McpManager {
    let mut config = ManagerConfig::new(ClientInfo::new("tester", "1"));
    config.http = Some(reqwest::Client::builder().no_proxy().build().unwrap());
    config.connect_timeout = Duration::from_secs(10);
    McpManager::new(config).unwrap()
}

pub(crate) fn remote(url: String, headers: RemoteHeaders) -> ServerConfig {
    ServerConfig::new(ServerSpec::Remote { url, headers })
}

pub(crate) fn local(command: &str, args: &[&str]) -> ServerConfig {
    ServerConfig::new(ServerSpec::Local {
        command: command.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        env: BTreeMap::new(),
        cwd: None,
    })
}

pub(crate) fn bearer(token: &str) -> RemoteHeaders {
    RemoteHeaders::Static(vec![(
        "authorization".to_owned(),
        format!("Bearer {token}"),
    )])
}

pub(crate) async fn settled(manager: &McpManager) {
    tokio::time::timeout(Duration::from_secs(10), manager.settle())
        .await
        .unwrap();
}

pub(crate) fn status_of(manager: &McpManager, name: &str) -> ServerStatus {
    manager.status().get(name).cloned().unwrap()
}
