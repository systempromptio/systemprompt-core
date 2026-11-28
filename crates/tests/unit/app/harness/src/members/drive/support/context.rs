use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use systemprompt_identifiers::engine::IdClock;

use async_trait::async_trait;
use serde_json::{Map, Value};
use systemprompt_device::accounts::{Accounts, AccountsConfig, Browser, BrowserError};
use systemprompt_device::client::SpClient;
use systemprompt_device::protocol::ids::{AccountId, InstallId, InstanceKey};
use systemprompt_engine::config::{Layout, Paths};
use systemprompt_engine::tools::{PermissionGate, PermissionRequest, ToolContext, ToolError};
use systemprompt_harness::extension::{
    AppliedInstance, Enablement, ExtensionId, Handles, HostInfo, SettingsAccess, SpContext, SpPaths,
};
use systemprompt_harness::projection::{Applied, Counts, Gateway};
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
        sealer: Arc::new(systemprompt_device::secure::MemorySealer::for_tests()),
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
        session_id: systemprompt_identifiers::engine::SessionId::mint(&IdClock::new()),
        message_id: systemprompt_identifiers::engine::MessageId::mint(&IdClock::new()),
        call_id: systemprompt_identifiers::engine::CallId::try_new("call-1").unwrap(),
        agent: "build".to_owned(),
        cwd: PathBuf::from("."),
        worktree: PathBuf::from("."),
        data_dir: PathBuf::from("."),
        cancel: systemprompt_traits::CancellationToken::new(),
        gate,
        observer: None,
        progress: None,
        files: systemprompt_engine::tools::FileLocks::default(),
    }
}
