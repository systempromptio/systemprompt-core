//! The applied instance the tests govern with, and a governance rig over the
//! fake token source and hook routes.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_identifiers::ModelId;
use systemprompt_identifiers::engine::IdClock;

use serde_json::json;
use systemprompt_device::client::CredentialSource;
use systemprompt_device::protocol::ids::InstanceKey;
use systemprompt_engine::run::{ModelCall, Purpose, ToolCall};
use systemprompt_harness::governance::{Governance, GovernanceTelemetry, HookApi, InstanceParts};
use systemprompt_harness::projection::{
    Applied, Counts, Gateway, GatewayModel, ManagedMcp, Protocol as Route,
};
use systemprompt_identifiers::engine::{CallId, SessionId};
use systemprompt_identifiers::{McpServerId, PluginId, SkillId};
use systemprompt_traits::CancellationToken;
use url::Url;

use crate::governance::fake_api::{FakeApi, FakeCredentials};

pub(crate) const INSTANCE: &str = "https://sp.example.com";
pub(crate) const HEADER: &str = "eyJhbGciOiJub25lIn0";
pub(crate) const VALID: &str = "eyJleHAiOjQxMDI0NDQ4MDB9";
pub(crate) const EXPIRED: &str = "eyJleHAiOjEwMDB9";

fn gateway() -> Gateway {
    Gateway {
        base_url: Some(format!("{INSTANCE}/v1")),
        models: vec![
            GatewayModel {
                id: "claude-sonnet-5-5".to_owned(),
                name: "Claude Sonnet 5.5".to_owned(),
                protocol: Route::Messages,
                context: Some(1_000_000),
                output: Some(128_000),
                families: vec!["claude".to_owned()],
            },
            GatewayModel {
                id: "gpt-x".to_owned(),
                name: String::new(),
                protocol: Route::ChatCompletions,
                context: None,
                output: None,
                families: vec!["openai".to_owned()],
            },
        ],
        default_model: None,
        protocols: vec!["claude".to_owned(), "openai".to_owned()],
    }
}

fn servers() -> Vec<ManagedMcp> {
    let mut declared = BTreeMap::new();
    declared.insert("x-team".to_owned(), "alpha".to_owned());
    vec![
        ManagedMcp {
            id: McpServerId::new("docs"),
            name: "docs".to_owned(),
            url: format!("{INSTANCE}/api/v1/mcp/docs/mcp"),
            bearer: true,
            headers: declared.clone(),
            tool_policy: BTreeMap::new(),
        },
        ManagedMcp {
            id: McpServerId::new("other"),
            name: "other".to_owned(),
            url: "https://tools.example.org/mcp".to_owned(),
            bearer: false,
            headers: declared,
            tool_policy: BTreeMap::new(),
        },
    ]
}

pub(crate) fn applied(governed: bool) -> Applied {
    let mut owners = BTreeMap::new();
    owners.insert(SkillId::new("alpha_notes"), PluginId::new("alpha-tools"));
    Applied {
        instance_url: INSTANCE.to_owned(),
        instance_key: InstanceKey::new("alpha"),
        gateway: gateway(),
        mcp: servers(),
        governance_plugin: governed.then(|| PluginId::new("alpha-governance")),
        skill_owners: owners,
        publications: Vec::new(),
        default_theme: None,
        counts: Counts::default(),
    }
}

pub(crate) struct Rig {
    pub(crate) governance: Arc<Governance>,
    pub(crate) telemetry: GovernanceTelemetry,
    pub(crate) api: Arc<FakeApi>,
    pub(crate) credentials: Arc<FakeCredentials>,
}

impl Rig {
    pub(crate) fn parts(&self, governed: bool, organisation: Option<&str>) -> InstanceParts {
        let api: Arc<dyn HookApi> = Arc::<FakeApi>::clone(&self.api);
        let credentials: Arc<dyn CredentialSource> =
            Arc::<FakeCredentials>::clone(&self.credentials);
        InstanceParts {
            applied: applied(governed),
            api,
            base: Url::parse(INSTANCE).unwrap(),
            credentials,
            organisation: organisation.map(str::to_owned),
            extensions: None,
        }
    }

    pub(crate) fn reinstall(&self) {
        self.governance.install(self.parts(true, None)).unwrap();
    }
}

pub(crate) fn rig(governed: bool, lifetime_secs: i64) -> Rig {
    let governance = Governance::new(&CancellationToken::new());
    let rig = Rig {
        telemetry: GovernanceTelemetry::new(Arc::clone(&governance)),
        governance,
        api: Arc::new(FakeApi::new(lifetime_secs)),
        credentials: Arc::new(FakeCredentials::default()),
    };
    rig.governance
        .install(rig.parts(governed, Some("Alpha Org")))
        .unwrap();
    rig
}

pub(crate) fn call(model: &str) -> ModelCall {
    ModelCall {
        session_id: SessionId::mint(&IdClock::new()),
        model_id: ModelId::new(model),
        purpose: Purpose::Step,
        directory: PathBuf::from("/work"),
    }
}

pub(crate) fn tool(session: &SessionId) -> ToolCall {
    ToolCall {
        session_id: session.clone(),
        call_id: CallId::try_new("call_1").unwrap(),
        tool: "bash".to_owned(),
        input: json!({ "command": "ls" }),
        directory: PathBuf::from("/work"),
        skill: None,
    }
}

pub(crate) fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}
