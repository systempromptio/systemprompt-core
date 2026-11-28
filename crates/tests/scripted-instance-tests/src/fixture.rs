use std::path::PathBuf;
use std::time::Duration;

use systemprompt_device::accounts::Accounts;
use systemprompt_device::client::{Freshness, HttpClient, SpClient};
use systemprompt_device::protocol::Jwt;
use systemprompt_device::protocol::ids::AccountId;
use systemprompt_golden_runner::host::host;
use systemprompt_golden_runner::relay::Relay;
use systemprompt_harness::command::{
    Composition, DataRoot, HeadlessHost, LoginReport, LoginRequest, login, read_token,
};
use systemprompt_scripted_instance::{ModelFixture, ModelScript, ScriptedInstance};
use tempfile::TempDir;

pub(crate) const MODEL: &str = "scripted-model";
pub(crate) const TITLE: &str = "Scripted title";
const APPLY_DEADLINE: Duration = Duration::from_secs(30);

pub(crate) fn http() -> HttpClient {
    HttpClient::new("scripted-instance-tests").unwrap()
}

/// A scripted instance behind the title relay, and a data root signed in to
/// it through the headless `login`.
pub(crate) struct Signed {
    pub(crate) instance: ScriptedInstance,
    pub(crate) relay: Relay,
    pub(crate) host: HeadlessHost,
    pub(crate) root: PathBuf,
    pub(crate) report: LoginReport,
    pub(crate) accounts: Accounts,
    pub(crate) account: AccountId,
    pub(crate) client: SpClient,
    pub(crate) dir: TempDir,
}

impl Signed {
    pub(crate) async fn start(script: ModelScript) -> Self {
        let instance = ScriptedInstance::builder()
            .models([ModelFixture::messages(MODEL)])
            .script(script)
            .start()
            .await
            .unwrap();
        Self::sign_in(instance).await
    }

    pub(crate) async fn sign_in(instance: ScriptedInstance) -> Self {
        let relay = Relay::start(instance.base_url(), TITLE, "claude")
            .await
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let host = host(dir.path());
        let root = dir.path().join("root");
        let token = read_token(&mut instance.pat().as_str().as_bytes()).unwrap();
        let report = login(
            &host,
            LoginRequest {
                instance: relay.url().to_owned(),
                token,
                fingerprint: None,
                data_root: Some(root.clone()),
            },
        )
        .await;
        assert!(report.status.is_success(), "{}", report.encode().unwrap());
        let accounts = Composition::open_accounts(&host, &host.root(Some(&root))).unwrap();
        let account = accounts.active_id().expect("no active account after login");
        let client = accounts.client(&account).unwrap();
        Self {
            instance,
            relay,
            host,
            root,
            report,
            accounts,
            account,
            client,
            dir,
        }
    }

    pub(crate) fn data_root(&self) -> DataRoot {
        self.host.root(Some(&self.root))
    }

    pub(crate) async fn jwt(&self) -> Jwt {
        self.accounts
            .credentials(&self.account)
            .jwt(Freshness::ValidFor(Duration::from_secs(60)))
            .await
            .unwrap()
    }

    /// The window's composition over the signed-in root, once the instance
    /// is applied.
    pub(crate) async fn governed(&self) -> Composition {
        let composition = Composition::start(&self.host, &self.data_root()).unwrap();
        let applied = tokio::time::timeout(APPLY_DEADLINE, async {
            while composition.governance.applied().is_none() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        assert!(
            applied.is_ok(),
            "no instance was applied within {APPLY_DEADLINE:?}"
        );
        composition
    }

    pub(crate) fn workspace(&self) -> PathBuf {
        let work = self.dir.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        work
    }
}
