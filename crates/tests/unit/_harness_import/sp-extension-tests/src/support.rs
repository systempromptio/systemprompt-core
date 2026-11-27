//! A context over a throwaway accounts store, in-memory settings, and small
//! extensions to resolve.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use harness_config::{Layout, Paths};
use serde_json::{Map, Value};
use sp_accounts::{Accounts, AccountsConfig, Browser, BrowserError};
use sp_client::HttpClient;
use sp_extension::{
    Enablement, ExtensionId, ExtensionMetadata, Handles, HostInfo, SettingsAccess, SpContext,
    SpExtension, SpPaths,
};
use sp_protocol::ids::InstallId;
use tokio::sync::watch;
use url::Url;

#[derive(Debug)]
struct NoBrowser;

impl Browser for NoBrowser {
    fn open(&self, _url: &Url) -> Result<(), BrowserError> {
        Ok(())
    }
}

pub(crate) struct MemorySettings {
    values: Mutex<std::collections::BTreeMap<ExtensionId, Map<String, Value>>>,
    changes: watch::Sender<u64>,
}

impl MemorySettings {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            values: Mutex::new(std::collections::BTreeMap::new()),
            changes: watch::channel(0).0,
        })
    }
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

fn engine_paths(root: &Path) -> Paths {
    Paths {
        home: root.join("home"),
        data: root.join("data"),
        config: root.join("config"),
        cache: root.join("cache"),
        state: root.join("state"),
        tmp: root.join("tmp"),
    }
}

pub(crate) fn context(root: &Path) -> SpContext {
    let http = HttpClient::new("harness-tests/0").unwrap();
    let accounts = Accounts::open(AccountsConfig {
        http: http.clone(),
        sealer: Arc::new(sp_secure::MemorySealer::for_tests()),
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
            engine: engine_paths(root),
            layout: Layout::default(),
            state: root.join("state"),
            cache: root.join("cache"),
            documents: None::<PathBuf>,
        },
        accounts,
        http,
        settings: MemorySettings::new(),
        handles: Handles::default(),
        enablement: Enablement::new(),
    }
}

pub(crate) fn id(value: &str) -> ExtensionId {
    ExtensionId::parse(value).unwrap()
}

pub(crate) struct Plain(pub(crate) ExtensionMetadata);

impl SpExtension for Plain {
    fn metadata(&self) -> ExtensionMetadata {
        self.0.clone()
    }
}

pub(crate) fn plain(value: &str, priority: u32) -> Arc<dyn SpExtension> {
    Arc::new(Plain(
        ExtensionMetadata::new(id(value), "plain").with_priority(priority),
    ))
}
