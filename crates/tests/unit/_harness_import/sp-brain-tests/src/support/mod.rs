//! A fake instance behind the brain routes, a context over a throwaway
//! accounts store, and the wire values the instance answers.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use harness_config::{Layout, Paths};
use harness_tools::{PermissionGate, PermissionRequest, ToolContext, ToolError};
use serde_json::{Map, Value, json};
use sp_accounts::{Accounts, AccountsConfig, Browser, BrowserError};
use sp_client::{CredentialError, CredentialSource, Freshness, HttpClient, SpClient};
use sp_extension::{
    AppliedInstance, CancellationToken, EnabledExtensions, Enablement, ExtensionId, Handles,
    HostInfo, SettingsAccess, SpContext, SpPaths,
};
use sp_projection::{Applied, Counts, Gateway};
use sp_protocol::Jwt;
use sp_protocol::ids::{AccountId, InstallId, InstanceKey};
use tokio::sync::watch;
use url::Url;

mod instance;
mod rig;

pub(crate) use instance::{FakeInstance, FakeRequest, FakeResponse};
pub(crate) use rig::{Rig, listing};

pub(crate) const FACT: &str = "m_6f1c2b8e-1d2a-4c3b-9e8f-0a1b2c3d4e5f";
pub(crate) const FACT2: &str = "m_7a2d3c9f-2e3b-4d4c-8f90-1b2c3d4e5f60";
pub(crate) const REVISION: &str = "11111111-2222-4333-8444-555555555555";
pub(crate) const DISPUTE: &str = "22222222-3333-4444-8555-666666666666";
pub(crate) const PROPOSAL: &str = "33333333-4444-4555-8666-777777777777";
pub(crate) const OUTBOX: &str = "44444444-5555-4666-8777-888888888888";
pub(crate) const USER: &str = "1ecce4e3-bc87-441d-b374-ea29bab88da8";
pub(crate) const AT: &str = "2026-10-30T10:00:00Z";

pub(crate) fn http() -> HttpClient {
    HttpClient::new("sp-brain-tests/0.0.0").unwrap()
}

pub(crate) fn fact_json(id: &str, status: &str) -> Value {
    json!({
        "id": id,
        "kind": "client_contact",
        "subject": { "kind": "client", "id": "acme" },
        "scope": { "level": "project", "key": "sales" },
        "statement": "Acme's primary contact is Jane Doe",
        "fields": { "contact": "Jane Doe" },
        "source": { "type": "entered_by", "user_id": USER },
        "topics": ["accounts"],
        "status": status,
        "confirmed_by": null,
        "confirmed_at": null,
        "importance_base": 0.5,
        "observed_at": AT,
        "valid_from": null,
        "valid_to": null,
        "review_after": null,
        "retention_until": null,
        "legal_hold": false,
        "identity_hash": "ih",
        "statement_hash": "sh",
        "revision": 3,
        "pinned": false,
        "owner_user_id": null,
        "usage_cited_rate": 0.5,
        "usage_ignored_rate": 0.5,
        "created_at": AT,
        "updated_at": AT
    })
}

pub(crate) fn revision_json(change: &str, status: &str) -> Value {
    json!({
        "id": REVISION,
        "fact_id": FACT,
        "revision": 4,
        "change": change,
        "actor": USER,
        "status": status,
        "statement": "Acme's primary contact is Jane Doe",
        "fields": { "contact": "Jane Doe" },
        "recorded_at": AT
    })
}

pub(crate) fn error_json(key: &str) -> Value {
    json!({ "code": "not_found", "message": "refused", "error_key": key, "timestamp": AT })
}

#[derive(Debug, Default)]
pub(crate) struct CountingJwt {
    pub(crate) minted: AtomicUsize,
    pub(crate) rejected: AtomicUsize,
}

#[async_trait]
impl CredentialSource for CountingJwt {
    async fn jwt(&self, _freshness: Freshness) -> Result<Jwt, CredentialError> {
        let n = self.minted.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(Jwt::new(format!("header.claims.sig{n}")).unwrap())
    }

    async fn rejected(&self) {
        self.rejected.fetch_add(1, Ordering::SeqCst);
    }
}

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
    let http = http();
    let accounts = Accounts::open(AccountsConfig {
        http: http.clone(),
        sealer: Arc::new(sp_secure::MemorySealer::for_tests()),
        state_file: root.join("accounts.json"),
        product: "Brain Tests".to_owned(),
        install_id: InstallId::new("install-1"),
        browser: Arc::new(NoBrowser),
        device_name: "Brain Tests on WS-1".to_owned(),
    })
    .unwrap();
    SpContext {
        host: HostInfo {
            product: "Brain Tests",
            display_name: "Brain Tests",
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
        http,
        settings: Arc::new(MemorySettings {
            values: Mutex::new(std::collections::BTreeMap::new()),
            changes: watch::channel(0).0,
        }),
        handles: Handles::default(),
        enablement: Enablement::new(),
    }
}

pub(crate) fn applied(url: &str) -> AppliedInstance {
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
        credentials: Arc::new(CountingJwt::default()),
        organisation: None,
        extensions: Some(EnabledExtensions::from_iter([sp_brain::BRAIN_EXTENSION])),
    }
}

#[derive(Default)]
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
        cancel: CancellationToken::new(),
        gate,
        observer: None,
        progress: None,
    }
}

pub(crate) async fn eventually(what: &str, mut check: impl FnMut() -> bool) {
    for _ in 0..400 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("this never happened: {what}");
}
