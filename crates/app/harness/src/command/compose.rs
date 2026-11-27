//! The composition a headless command runs in: what the window's composition
//! root builds, without a window.
//!
//! Inside the tokio runtime, in this order: the directories, the session
//! store, the instance HTTP client, the accounts, governance installed into
//! the [`GovernorSlot`] and inserted into the shared handles, the registered
//! extensions provided (dependencies first), the engine built from their
//! engine halves, then the live sync with their manifest appliers. Shutdown
//! runs in the reverse order of dependence: sync, governance cleared, engine,
//! accounts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_device::accounts::{
    Accounts, AccountsConfig, AccountsError, Browser, BrowserError,
};
use systemprompt_device::client::{HttpClient, HttpClientError};
use systemprompt_device::protocol::ids::InstallId;
use systemprompt_engine::config::Layout;
use systemprompt_engine::llm::{HttpTransport, TransportError};
use systemprompt_engine::mcp::ClientInfo;
use systemprompt_engine::run::{Engine, EngineError, EngineOptions, Governor};
use systemprompt_engine::store::{Store, StoreError};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_traits::CancellationToken;
use url::Url;

use crate::command::host::{DataRoot, HeadlessHost};
use crate::command::slot::GovernorSlot;
use crate::extension::{Enablement, ExtensionError, Handles, SpContext, SpExtensions, SpPaths};
use crate::governance::Governance;
use crate::projection::Applier;
use crate::sync::{SyncDeps, SyncEngine, SyncError};

/// Why the headless composition could not be started.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("the directory {} could not be created", path.display())]
    Directories {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the session store could not be opened")]
    Store(#[source] StoreError),
    #[error("the instance HTTP client could not be built")]
    InstanceClient(#[source] HttpClientError),
    #[error("the model HTTP client could not be built")]
    ModelClient(#[source] TransportError),
    #[error("the accounts could not be opened")]
    Accounts(#[source] AccountsError),
    #[error("the registered extensions could not be assembled")]
    Extensions(#[source] ExtensionError),
    #[error("the engine could not be started")]
    Engine(#[source] EngineError),
    #[error("the live sync could not be started")]
    Sync(#[source] SyncError),
}

#[derive(Debug, Clone, Copy)]
struct NoBrowser;

impl Browser for NoBrowser {
    fn open(&self, _url: &Url) -> Result<(), BrowserError> {
        Err(BrowserError(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "a headless command opens no browser",
        )))
    }
}

/// The running pieces of one headless command.
#[derive(Debug)]
pub struct Composition {
    pub accounts: Accounts,
    pub store: Store,
    pub engine: Engine,
    pub governance: Arc<Governance>,
    pub sync: SyncEngine,
    cancel: CancellationToken,
}

fn layout(host: &HeadlessHost) -> Layout {
    Layout {
        project_dir: host.project_dir.clone(),
        ..Layout::default()
    }
}

fn instance_client(host: &HeadlessHost) -> Result<HttpClient, StartupError> {
    let user_agent = format!("{}/{}", HostKind::Harness.as_str(), host.info.version);
    HttpClient::new(&user_agent).map_err(StartupError::InstanceClient)
}

fn ensure(root: &DataRoot) -> Result<(), StartupError> {
    root.ensure().map_err(|source| StartupError::Directories {
        path: root.root.clone(),
        source,
    })
}

fn accounts(
    host: &HeadlessHost,
    root: &DataRoot,
    http: &HttpClient,
) -> Result<Accounts, StartupError> {
    Accounts::open(AccountsConfig {
        http: http.clone(),
        sealer: Arc::clone(&host.sealer),
        state_file: root.accounts_file(),
        product: host.info.product.to_owned(),
        install_id: InstallId::new(uuid::Uuid::new_v4().to_string()),
        browser: Arc::new(NoBrowser),
        device_name: host.device_name.clone(),
    })
    .map_err(StartupError::Accounts)
}

fn client_info(host: &HeadlessHost) -> ClientInfo {
    let mut info = ClientInfo::new(HostKind::Harness.as_str(), host.info.version);
    info.title = Some(host.info.display_name.to_owned());
    info
}

impl Composition {
    pub fn open_accounts(host: &HeadlessHost, root: &DataRoot) -> Result<Accounts, StartupError> {
        ensure(root)?;
        let http = instance_client(host)?;
        accounts(host, root, &http)
    }

    pub fn start(host: &HeadlessHost, root: &DataRoot) -> Result<Self, StartupError> {
        ensure(root)?;
        let layout = layout(host);
        let store = Store::open(root.store_file()).map_err(StartupError::Store)?;
        let http = instance_client(host)?;
        let accounts = accounts(host, root, &http)?;
        let cancel = CancellationToken::new();

        let slot = Arc::new(GovernorSlot::default());
        let governance = Governance::new(&cancel);
        governance.set_verdicts_enabled(host.verdicts_enabled);
        let installed: Arc<dyn Governor> = Arc::<Governance>::clone(&governance);
        slot.install(installed);

        let handles = Handles::default();
        handles.insert(Arc::clone(&governance));
        let context = SpContext {
            host: host.info,
            paths: SpPaths {
                engine: root.engine_paths(),
                layout: layout.clone(),
                state: root.state.clone(),
                cache: root.cache.clone(),
                documents: host.documents.clone(),
            },
            accounts: accounts.clone(),
            http,
            settings: Arc::clone(&host.settings),
            handles,
            enablement: Enablement::new(),
        };
        let extensions = SpExtensions::from_registrations(&host.registrations)
            .map_err(StartupError::Extensions)?;
        extensions.provide(&context);

        let transport = HttpTransport::new().map_err(StartupError::ModelClient)?;
        let engine_governor: Arc<dyn Governor> = slot;
        let engine = Engine::new(EngineOptions {
            paths: root.engine_paths(),
            layout: layout.clone(),
            store: store.clone(),
            governor: engine_governor,
            transport: Arc::new(transport),
            client_info: client_info(host),
            search_path: host.search_path.clone(),
            path_ext: host.path_ext.clone(),
            extensions: extensions
                .engine_set(&context)
                .map_err(StartupError::Extensions)?,
            lsp_client_name: Some(HostKind::Harness.as_str().to_owned()),
        })
        .map_err(StartupError::Engine)?;

        let sync = SyncEngine::start(SyncDeps {
            accounts: accounts.clone(),
            governance: Arc::clone(&governance),
            appliers: extensions.appliers(&context),
            applier: Applier::new(root.engine_paths(), layout),
            state_dir: root.state.clone(),
            cache_dir: root.cache.clone(),
            release_version: host.info.version.to_owned(),
            cancel: cancel.child_token(),
        })
        .map_err(StartupError::Sync)?;
        context.handles.insert(Arc::new(sync.clone()));
        tracing::info!(root = %root.root.display(), "headless composition started");
        Ok(Self {
            accounts,
            store,
            engine,
            governance,
            sync,
            cancel,
        })
    }

    pub async fn shutdown(&self) {
        self.sync.shutdown().await;
        self.governance.clear();
        self.engine.shutdown().await;
        self.accounts.shutdown();
        self.cancel.cancel();
        tracing::info!("headless composition stopped");
    }
}
