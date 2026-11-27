use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use harness_config::{Layout, Paths};
use harness_tools::{PermissionGate, PermissionRequest, ToolContext, ToolError};
use serde_json::{Map, Value};
use sp_accounts::{Accounts, AccountsConfig, Browser, BrowserError};
use sp_client::SpClient;
use sp_extension::{
    AppliedInstance, Enablement, ExtensionId, Handles, HostInfo, SettingsAccess, SpContext, SpPaths,
};
use sp_projection::{Applied, Counts, Gateway};
use sp_protocol::ids::{AccountId, InstallId, InstanceKey};
use tokio::sync::watch;
use url::Url;

use super::{StaticJwt, http};

#[derive(Debug)]
struct NoBrowser;

impl Browser for NoBrowser {
    fn open(&self, _url: &Url) -> Result<(), BrowserError> {
        Ok(())
    }
}

struct MemorySettings {
    values: Mutex<BTreeMap<ExtensionId, Map<String, Value>>>,
    changes: watch::Sender<u64>,
}

impl SettingsAccess for MemorySettings {
    fn read(&self, extension: &ExtensionId) -> Option<Map<String, Value>> {
        self.values.lock().unwrap().get(extension).cloned()
    }

    fn write(&self, extension: &ExtensionId, values: Map<String, Value>) {
        self.values
            .lock()
            .unwrap()
            .insert(extension.clone(), values);
        self.changes.send_modify(|count| *count += 1);
    }

    fn changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }
}

pub(crate) fn context(root: &Path, documents: Option<PathBuf>) -> SpContext {
    let accounts = Accounts::open(AccountsConfig {
        http: http(),
        sealer: Arc::new(sp_secure::MemorySealer::for_tests()),
        state_file: root.join("accounts.json"),
        product: "Drive Tests".to_owned(),
        install_id: InstallId::new("install-1"),
        browser: Arc::new(NoBrowser),
        device_name: "Drive Tests on WS-1".to_owned(),
    })
    .unwrap();
    SpContext {
        host: HostInfo {
            product: "Drive Tests",
            display_name: "Drive Tests",
            version: "0.0.0",
        },
        paths: SpPaths {
            engine: Paths {
                home: root.join("home"),
                data: root.join("data"),
                config: root.join("config"),
                cache: root.join("cache"),
                state: root.join("state"),
                tmp: root.join("tmp"),
            },
            layout: Layout::default(),
            state: root.join("state"),
            cache: root.join("cache"),
            documents,
        },
        accounts,
        http: http(),
        settings: Arc::new(MemorySettings {
            values: Mutex::new(BTreeMap::new()),
            changes: watch::channel(0).0,
        }),
        handles: Handles::default(),
        enablement: Enablement::new(),
    }
}

pub(crate) fn applied(url: &str, account: &str) -> AppliedInstance {
    AppliedInstance {
        account: AccountId::new(account),
        applied: Applied {
            instance_url: url.to_owned(),
            instance_key: InstanceKey::new("alpha"),
            gateway: Gateway::default(),
            mcp: Vec::new(),
            governance_plugin: None,
            skill_owners: BTreeMap::new(),
            publications: Vec::new(),
            default_theme: None,
            counts: Counts::default(),
        },
        client: SpClient::new(url, http()).unwrap(),
        credentials: Arc::new(StaticJwt),
        organisation: None,
        extensions: None,
    }
}

#[derive(Debug, Default)]
pub(crate) struct Gate {
    pub(crate) asked: Mutex<Vec<PermissionRequest>>,
}

#[async_trait]
impl PermissionGate for Gate {
    async fn ask(&self, request: PermissionRequest) -> Result<(), ToolError> {
        self.asked.lock().unwrap().push(request);
        Ok(())
    }
}

pub(crate) fn tool_context(gate: Arc<Gate>) -> ToolContext {
    ToolContext {
        session_id: harness_ids::SessionId::mint(),
        message_id: harness_ids::MessageId::mint(),
        call_id: harness_ids::CallId::try_new("call-1").unwrap(),
        agent: "build".to_owned(),
        cwd: PathBuf::from("."),
        worktree: PathBuf::from("."),
        data_dir: PathBuf::from("."),
        cancel: harness_tasks::CancellationToken::new(),
        gate,
        observer: None,
        progress: None,
    }
}
