use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value};
use systemprompt_device::accounts::{Accounts, AccountsConfig, Browser, BrowserError};
use systemprompt_device::client::SpClient;
use systemprompt_device::protocol::ids::{AccountId, InstallId, InstanceKey};
use systemprompt_engine::config::{Layout, Paths};
use systemprompt_harness::extension::{
    AppliedInstance, EnabledExtensions, Enablement, ExtensionId, Handles, HostInfo, SettingsAccess,
    SpContext, SpPaths,
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
    values: Mutex<std::collections::BTreeMap<ExtensionId, Map<String, Value>>>,
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

pub(crate) fn context(root: &Path) -> SpContext {
    let accounts = Accounts::open(AccountsConfig {
        http: http(),
        sealer: Arc::new(systemprompt_device::secure::MemorySealer::for_tests()),
        state_file: root.join("accounts.json"),
        product: "Harness Tests".to_owned(),
        install_id: InstallId::new("install-1"),
        browser: Arc::new(NoBrowser),
        device_name: "Harness Tests on WS-1".to_owned(),
    })
    .unwrap();
    SpContext {
        host: HostInfo {
            product: "Harness Tests",
            display_name: "Harness Tests",
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
            documents: None::<PathBuf>,
        },
        accounts,
        http: http(),
        settings: Arc::new(MemorySettings {
            values: Mutex::new(std::collections::BTreeMap::new()),
            changes: watch::channel(0).0,
        }),
        handles: Handles::default(),
        enablement: Enablement::new(),
    }
}

pub(crate) fn enabled(ids: &[&str]) -> EnabledExtensions {
    ids.iter()
        .map(|id| ExtensionId::parse(id).unwrap())
        .collect()
}

pub(crate) fn applied(url: &str, extensions: Option<EnabledExtensions>) -> AppliedInstance {
    AppliedInstance {
        account: AccountId::new("account-1"),
        applied: Applied {
            instance_url: url.to_owned(),
            instance_key: InstanceKey::new("alpha"),
            gateway: Gateway::default(),
            mcp: Vec::new(),
            governance_plugin: None,
            skill_owners: std::collections::BTreeMap::new(),
            publications: Vec::new(),
            default_theme: None,
            counts: Counts::default(),
        },
        client: SpClient::new(url, http()).unwrap(),
        credentials: Arc::new(StaticJwt),
        organisation: None,
        extensions,
    }
}
