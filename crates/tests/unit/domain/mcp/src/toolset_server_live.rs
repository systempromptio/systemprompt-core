use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rmcp::model::{
    CallToolResult, ListResourcesResult, ReadResourceRequestParams, ReadResourceResult, Resource,
    ResourceContents, Tool,
};
use rmcp::{ErrorData as McpError, ServerHandler};
use systemprompt_governance::authz::{
    AllowAllHook, AuthzError, DenyAllHook, SharedAuthzHook, SubjectAttributeProvider,
    SubjectDimension, SubjectProviderSet,
};
use systemprompt_identifiers::{McpServerId, UserId};
use systemprompt_mcp::host::{
    McpAccessEvent, McpAccessRecorder, ToolsetAuthz, ToolsetHostError, ToolsetServer,
    ToolsetServerParts,
};
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_mcp::{ArtifactIngest, McpToolExecutor, McpToolset, ToolsetCall};
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::authz::RuleType;
use systemprompt_test_fixtures::{FIXTURE_EXTERNAL_URL, mint_bridge_jwt, mint_mcp_hop};

use crate::harness::bootstrap_with_services;

pub(crate) const SERVER: &str = "toolbox_live";

fn services_yaml() -> String {
    format!(
        "mcp_servers:\n  {SERVER}:\n    server_type: internal\n    binary: toolbox-bin\n    \
         package: null\n    port: 5391\n    enabled: true\n    display_in_web: false\n    \
         description: Toolbox Live\n    oauth:\n      required: true\n      scopes: [user]\n      \
         audience: mcp\n      client_id: null\n    toolsets:\n      - id: issues\n      - id: \
         admin\n        access:\n          scopes: [admin]\n    tools:\n      grant_role:\n        \
         access:\n          groups: [platform]\n"
    )
}

#[derive(Debug)]
struct FakeToolset {
    id: &'static str,
    tools: &'static [&'static str],
    resources: bool,
}

#[async_trait]
impl McpToolset for FakeToolset {
    fn id(&self) -> &str {
        self.id
    }

    fn instructions(&self) -> &'static str {
        if self.resources {
            "Use the issue tools."
        } else {
            "  "
        }
    }

    fn tools(&self) -> Vec<Tool> {
        self.tools
            .iter()
            .map(|name| Tool::new(*name, "fake", Arc::new(serde_json::Map::new())))
            .collect()
    }

    async fn call(&self, call: ToolsetCall<'_>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::structured(serde_json::json!({
            "toolset": self.id,
            "tool": call.request.name.as_ref(),
            "user": call.context.user.as_ref().map(|u| u.email.clone()),
        })))
    }

    fn serves_resources(&self) -> bool {
        self.resources
    }

    fn list_resources(&self) -> ListResourcesResult {
        ListResourcesResult::with_all_items(vec![Resource::new("issues://open", "open")])
    }

    fn read_resource(
        &self,
        request: &ReadResourceRequestParams,
    ) -> Option<Result<ReadResourceResult, McpError>> {
        (request.uri == "issues://open").then(|| {
            Ok(ReadResourceResult::new(vec![ResourceContents::text(
                "none open",
                request.uri.clone(),
            )]))
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct Captured(Mutex<Vec<String>>);

#[async_trait]
impl McpAccessRecorder for Captured {
    async fn record(&self, event: McpAccessEvent<'_>) {
        self.0.lock().expect("lock").push(format!(
            "{}:{}:{}",
            event.tool,
            event.outcome.action(),
            event.user_id.is_some()
        ));
    }
}

#[derive(Debug)]
struct Groups(Result<Vec<String>, ()>);

#[async_trait]
impl SubjectAttributeProvider for Groups {
    fn dimension(&self) -> SubjectDimension {
        SubjectDimension {
            rule_type: RuleType::extension_static("group"),
            label: "Group",
            precedence: 100,
        }
    }

    async fn values_for(&self, _user_id: &UserId) -> Result<Vec<String>, AuthzError> {
        self.0
            .clone()
            .map_err(|()| AuthzError::InvalidAccess("directory down".to_owned()))
    }
}

pub(crate) struct Fixture {
    pub(crate) server: ToolsetServer,
    pub(crate) recorded: Arc<Captured>,
}

impl Fixture {
    pub(crate) fn events(&self) -> Vec<String> {
        self.recorded.0.lock().expect("lock").clone()
    }
}

fn toolsets(resources: bool) -> Vec<Box<dyn McpToolset>> {
    vec![
        Box::new(FakeToolset {
            id: "issues",
            tools: &["report_issue", "issues_list"],
            resources,
        }),
        Box::new(FakeToolset {
            id: "admin",
            tools: &["users_list", "grant_role"],
            resources: false,
        }),
    ]
}

async fn parts(
    server: &str,
    hook: SharedAuthzHook,
    groups: Option<Result<Vec<String>, ()>>,
    sets: Vec<Box<dyn McpToolset>>,
) -> (ToolsetServerParts, Arc<Captured>) {
    let _bootstrap = bootstrap_with_services(&services_yaml());
    let db = systemprompt_test_fixtures::test_db_pool().await;
    let server_id = McpServerId::new(server);
    let executor = McpToolExecutor::new(
        Arc::new(ToolUsageRepository::new(&db)),
        Arc::new(systemprompt_ai::repository::AiRequestRepository::new(&db)),
        Arc::new(ArtifactIngest::from_db(&db, None)),
        server_id.clone(),
    );
    let subjects = groups.map_or_else(SubjectProviderSet::default, |values| {
        SubjectProviderSet::from_providers(vec![Arc::new(Groups(values))])
    });
    let recorded = Arc::new(Captured::default());
    let parts = ToolsetServerParts {
        server_id,
        executor,
        authz: ToolsetAuthz {
            hook,
            services: crate::harness::services_state().view(),
            subjects,
        },
        recorders: vec![Arc::clone(&recorded) as Arc<dyn McpAccessRecorder>],
        toolsets: sets,
    };
    (parts, recorded)
}

pub(crate) async fn fixture(
    hook: SharedAuthzHook,
    groups: Option<Result<Vec<String>, ()>>,
    resources: bool,
) -> Fixture {
    let (parts, recorded) = parts(SERVER, hook, groups, toolsets(resources)).await;
    Fixture {
        server: ToolsetServer::new(parts).expect("server builds"),
        recorded,
    }
}

pub(crate) fn allow() -> SharedAuthzHook {
    Arc::new(AllowAllHook::null())
}

pub(crate) fn deny() -> SharedAuthzHook {
    Arc::new(DenyAllHook::null())
}

fn caller(email: &str, permissions: Vec<Permission>) -> Vec<(String, String)> {
    let user = AuthenticatedUser::new(
        UserId::new(uuid::Uuid::new_v4().to_string()),
        email.to_owned(),
        email.to_owned(),
        permissions,
    );
    let relay = mint_bridge_jwt(&user.id, email, FIXTURE_EXTERNAL_URL);
    let hop = mint_mcp_hop(&McpServerId::new(SERVER), user, None, FIXTURE_EXTERNAL_URL);
    vec![
        ("x-systemprompt-mcp-hop".to_owned(), hop),
        (
            "authorization".to_owned(),
            format!("Bearer {}", relay.as_str()),
        ),
    ]
}

pub(crate) fn admin_caller() -> Vec<(String, String)> {
    caller("host-admin@test.invalid", vec![Permission::Admin])
}

pub(crate) fn user_caller() -> Vec<(String, String)> {
    caller("host-user@test.invalid", vec![Permission::User])
}

#[tokio::test]
async fn an_undeclared_server_is_refused_at_build() {
    let (parts, _) = parts("toolbox_absent", allow(), None, toolsets(false)).await;
    let err = ToolsetServer::new(parts).expect_err("unknown server");
    assert!(
        matches!(err, ToolsetHostError::UnknownServer { ref server } if server.as_str() == "toolbox_absent")
    );
}

#[tokio::test]
async fn the_server_reports_its_tools_title_and_instructions() {
    let fx = fixture(allow(), None, true).await;
    assert_eq!(fx.server.server_id().as_str(), SERVER);
    assert_eq!(
        fx.server.tool_names(),
        ["grant_role", "issues_list", "report_issue", "users_list"]
    );
    let info = fx.server.get_info();
    assert_eq!(info.server_info.title.as_deref(), Some("Toolbox Live"));
    assert_eq!(info.server_info.name, format!("Toolbox Live ({SERVER})"));
    assert_eq!(info.instructions.as_deref(), Some("Use the issue tools."));
    assert!(info.capabilities.resources.is_some());
}

#[tokio::test]
async fn a_server_without_resources_or_instructions_advertises_tools_only() {
    let fx = fixture(allow(), None, false).await;
    let info = fx.server.get_info();
    assert!(info.instructions.is_none());
    assert!(info.capabilities.resources.is_none());
    assert!(info.capabilities.tools.is_some());
}
