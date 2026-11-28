//! The instance and its builder.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use systemprompt_models::bridge::heartbeat::HEARTBEAT_INTERVAL_SECS;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

use crate::error::ScriptedInstanceError;
use crate::event::Event;
use crate::manifest::ManifestFixture;
use crate::model::ModelFixture;
use crate::record::RequestRecord;
use crate::router;
use crate::script::ModelScript;
use crate::state::{InstanceState, lock};
use crate::token::Keys;
use crate::user::{TestPat, TestUser};
use crate::verdict::VerdictScript;

const WAIT_TIMEOUT: Duration = Duration::from_secs(HEARTBEAT_INTERVAL_SECS + 5);

#[derive(Debug, Clone, Default)]
pub struct ScriptedInstanceBuilder {
    user: TestUser,
    pat: Option<TestPat>,
    manifest: ManifestFixture,
    models: Vec<ModelFixture>,
    verdict: VerdictScript,
    script: ModelScript,
    port: u16,
}

impl ScriptedInstanceBuilder {
    pub fn user(mut self, user: TestUser) -> Self {
        self.user = user;
        self
    }

    pub fn pat(mut self, pat: TestPat) -> Self {
        self.pat = Some(pat);
        self
    }

    pub fn manifest(mut self, manifest: ManifestFixture) -> Self {
        self.manifest = manifest;
        self
    }

    pub fn models(mut self, models: impl IntoIterator<Item = ModelFixture>) -> Self {
        self.models = models.into_iter().collect();
        self
    }

    pub fn verdict(mut self, verdict: VerdictScript) -> Self {
        self.verdict = verdict;
        self
    }

    pub fn script(mut self, script: ModelScript) -> Self {
        self.script = script;
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub async fn start(self) -> Result<ScriptedInstance, ScriptedInstanceError> {
        let port = self.port;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .await
            .map_err(|source| ScriptedInstanceError::Bind { port, source })?;
        let addr = listener
            .local_addr()
            .map_err(|source| ScriptedInstanceError::Bind { port, source })?;

        let base_url = format!("http://{addr}");
        let state = Arc::new(InstanceState {
            base_url: base_url.clone(),
            keys: Keys::generate(),
            hook_secret: format!("{:032x}", rand::random::<u128>()),
            arrivals: watch::Sender::new(0),
            codes: Mutex::new(Vec::new()),
            manifests_served: AtomicUsize::new(0),
            manifest_dated: Mutex::new(Utc::now()),
            user: self.user,
            pat: self.pat.unwrap_or_else(TestPat::fixture),
            manifest: self.manifest,
            models: self.models,
            verdict: self.verdict,
            script: self.script,
            requests: Mutex::new(Vec::new()),
            consumed_turns: AtomicUsize::new(0),
            consumer: Mutex::default(),
            drive: Mutex::default(),
        });

        let app = router::router(Arc::clone(&state));
        let (shutdown, stopped) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    if stopped.await.is_err() {
                        tracing::debug!("scripted instance dropped");
                    }
                })
                .await
        });

        tracing::info!(%addr, "scripted instance listening");
        Ok(ScriptedInstance {
            addr,
            base_url,
            state,
            server,
            shutdown,
        })
    }
}

#[derive(Debug)]
pub struct ScriptedInstance {
    addr: SocketAddr,
    base_url: String,
    state: Arc<InstanceState>,
    server: JoinHandle<std::io::Result<()>>,
    shutdown: oneshot::Sender<()>,
}

impl ScriptedInstance {
    pub fn builder() -> ScriptedInstanceBuilder {
        ScriptedInstanceBuilder::default()
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn pat(&self) -> &TestPat {
        &self.state.pat
    }

    pub fn user(&self) -> &TestUser {
        &self.state.user
    }

    pub fn manifest(&self) -> &ManifestFixture {
        &self.state.manifest
    }

    pub fn models(&self) -> &[ModelFixture] {
        &self.state.models
    }

    pub fn verdict(&self) -> &VerdictScript {
        &self.state.verdict
    }

    pub fn script(&self) -> &ModelScript {
        &self.state.script
    }

    pub fn requests(&self) -> Vec<RequestRecord> {
        lock(&self.state.requests).clone()
    }

    pub fn unscripted_requests(&self) -> Vec<RequestRecord> {
        let mut requests = self.requests();
        requests.retain(|record| record.unscripted);
        requests
    }

    pub fn unconsumed_turns(&self) -> usize {
        let consumed = self.state.consumed_turns.load(Ordering::SeqCst);
        self.state.script.turns.len().saturating_sub(consumed)
    }

    pub async fn wait_for(&self, event: Event) -> Result<RequestRecord, ScriptedInstanceError> {
        let mut arrivals = self.state.arrivals.subscribe();
        let arrived = async {
            loop {
                let recorded = self
                    .requests()
                    .into_iter()
                    .find(|record| event.matches(record));
                if let Some(record) = recorded {
                    return record;
                }
                if arrivals.changed().await.is_err() {
                    std::future::pending::<()>().await;
                }
            }
        };
        tokio::time::timeout(WAIT_TIMEOUT, arrived)
            .await
            .map_err(|_elapsed| ScriptedInstanceError::WaitTimedOut {
                event,
                waited: WAIT_TIMEOUT,
            })
    }

    pub async fn shutdown(self) -> Result<(), ScriptedInstanceError> {
        let Self {
            server, shutdown, ..
        } = self;
        drop(shutdown);
        server.await?.map_err(ScriptedInstanceError::Serve)
    }
}
