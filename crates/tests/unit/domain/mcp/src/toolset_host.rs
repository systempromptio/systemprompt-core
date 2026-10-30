// Declarative toolsets: access resolution (tool > toolset > server), YAML
// validation of `toolsets:` and `tools:` against what is hosted, and the
// per-caller tool filtering the generic host applies to `tools/list`.

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolResult, Tool};
use systemprompt_identifiers::{McpServerId, UserId};
use systemprompt_mcp::host::{HostedCatalog, ToolsetHostError, resolve_tool_access};
use systemprompt_mcp::{McpToolset, ToolsetCall};
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::mcp::{Deployment, ResolvedAccess, ToolAccess};

#[derive(Debug)]
struct FakeToolset {
    id: &'static str,
    tools: &'static [&'static str],
}

#[async_trait]
impl McpToolset for FakeToolset {
    fn id(&self) -> &str {
        self.id
    }

    fn tools(&self) -> Vec<Tool> {
        self.tools
            .iter()
            .map(|name| Tool::new(*name, "fake", Arc::new(serde_json::Map::new())))
            .collect()
    }

    async fn call(&self, _call: ToolsetCall<'_>) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![]))
    }
}

fn toolsets() -> Vec<Box<dyn McpToolset>> {
    vec![
        Box::new(FakeToolset {
            id: "issues",
            tools: &["report_issue", "issues_list"],
        }),
        Box::new(FakeToolset {
            id: "admin",
            tools: &["users_list", "grant_role"],
        }),
    ]
}

const BASE: &str = "binary: systemprompt-mcp-x\nport: 5099\nenabled: true\n\
display_in_web: false\noauth:\n  required: true\n  scopes: [user]\n  audience: mcp\n  \
client_id: null\n";

fn deployment(extra: &str) -> Deployment {
    serde_yaml::from_str(&format!("{BASE}{extra}")).expect("deployment yaml")
}

fn hosting() -> Deployment {
    deployment(
        "toolsets:\n  - id: issues\n  - id: admin\n    access:\n      scopes: [admin]\n\
tools:\n  grant_role:\n    access:\n      groups: [platform]\n  issues_list:\n    \
tool_policy: prompt\n",
    )
}

fn server() -> McpServerId {
    McpServerId::new("toolbox")
}

fn user(permissions: Vec<Permission>, roles: &[&str]) -> AuthenticatedUser {
    AuthenticatedUser::new_with_roles(
        UserId::new("user-1"),
        "user".to_owned(),
        "user@example.com".to_owned(),
        permissions,
        roles.iter().map(|r| (*r).to_owned()).collect(),
    )
}

fn names(tools: &[Tool]) -> Vec<String> {
    tools.iter().map(|t| t.name.to_string()).collect()
}

#[test]
fn tool_access_wins_over_toolset_access() {
    let d = hosting();
    let access = resolve_tool_access(&d, "admin", "grant_role");
    assert!(matches!(access, ResolvedAccess::Tool(a) if a.groups == ["platform"]));
}

#[test]
fn toolset_access_applies_when_the_tool_declares_none() {
    let d = hosting();
    let access = resolve_tool_access(&d, "admin", "users_list");
    assert!(matches!(access, ResolvedAccess::Toolset(a) if a.scopes == [Permission::Admin]));
}

#[test]
fn server_scopes_apply_when_neither_declares_access() {
    let d = hosting();
    assert_eq!(
        resolve_tool_access(&d, "issues", "issues_list"),
        ResolvedAccess::Server
    );
    assert!(ResolvedAccess::Server.permits(None, &[]));
}

#[test]
fn access_is_any_of_scopes_roles_and_groups() {
    let access = ToolAccess {
        scopes: vec![Permission::Admin],
        roles: vec!["auditor".to_owned()],
        groups: vec!["platform".to_owned()],
    };
    assert!(access.permits(Some(&user(vec![Permission::Admin], &[])), &[]));
    assert!(access.permits(Some(&user(vec![Permission::User], &["auditor"])), &[]));
    assert!(access.permits(
        Some(&user(vec![Permission::User], &[])),
        &["platform".to_owned()]
    ));
    assert!(!access.permits(Some(&user(vec![Permission::User], &[])), &[]));
    assert!(!access.permits(None, &["platform".to_owned()]));
}

#[test]
fn a_higher_scope_implies_a_lower_one() {
    let access = ToolAccess {
        scopes: vec![Permission::User],
        ..ToolAccess::default()
    };
    assert!(access.permits(Some(&user(vec![Permission::Admin], &[])), &[]));
}

#[test]
fn catalog_builds_the_union_of_hosted_toolsets() {
    let catalog = HostedCatalog::build(&server(), &hosting(), &toolsets()).expect("catalog");
    assert_eq!(
        catalog.names(),
        ["grant_role", "issues_list", "report_issue", "users_list"]
    );
}

#[test]
fn a_tools_entry_no_toolset_serves_fails_the_build() {
    let d = deployment("toolsets:\n  - id: issues\ntools:\n  not_a_tool:\n    tool_policy: deny\n");
    let err = HostedCatalog::build(&server(), &d, &toolsets()).expect_err("unhosted tool");
    assert!(matches!(err, ToolsetHostError::UnhostedTool { ref tool, .. } if tool == "not_a_tool"));
}

#[test]
fn two_toolsets_serving_one_tool_fail_the_build() {
    let duplicate: Vec<Box<dyn McpToolset>> = vec![
        Box::new(FakeToolset {
            id: "a",
            tools: &["shared"],
        }),
        Box::new(FakeToolset {
            id: "b",
            tools: &["shared"],
        }),
    ];
    let err = HostedCatalog::build(&server(), &deployment(""), &duplicate).expect_err("dup");
    assert!(matches!(err, ToolsetHostError::DuplicateTool { .. }));
}

#[test]
fn hosting_no_toolsets_fails_the_build() {
    let err = HostedCatalog::build(&server(), &deployment(""), &[]).expect_err("empty");
    assert!(matches!(err, ToolsetHostError::NoToolsets { .. }));
}

#[test]
fn a_plain_user_sees_only_server_scoped_tools() {
    let d = hosting();
    let sets = toolsets();
    let catalog = HostedCatalog::build(&server(), &d, &sets).expect("catalog");
    let visible = catalog.visible_tools(&sets, &d, Some(&user(vec![Permission::User], &[])), &[]);
    assert_eq!(names(&visible), ["issues_list", "report_issue"]);
}

#[test]
fn an_admin_outside_the_group_does_not_see_the_group_tool() {
    let d = hosting();
    let sets = toolsets();
    let catalog = HostedCatalog::build(&server(), &d, &sets).expect("catalog");
    let admin = user(vec![Permission::Admin], &[]);
    let visible = catalog.visible_tools(&sets, &d, Some(&admin), &[]);
    assert_eq!(
        names(&visible),
        ["issues_list", "report_issue", "users_list"]
    );
    assert!(catalog.needs_groups(&sets, &d));
    let in_group = catalog.visible_tools(&sets, &d, Some(&admin), &["platform".to_owned()]);
    assert_eq!(
        names(&in_group),
        ["grant_role", "issues_list", "report_issue", "users_list"]
    );
}

#[test]
fn toolsets_are_refused_on_external_servers() {
    let d: Deployment = serde_yaml::from_str(
        "server_type: external\nendpoint: https://mcp.example.com/mcp\nenabled: true\n\
display_in_web: false\noauth:\n  required: false\n  scopes: []\n  audience: mcp\n  \
client_id: null\ntoolsets:\n  - id: issues\n",
    )
    .expect("yaml");
    assert!(d.validate("ext").is_err());
}

#[test]
fn a_toolset_listed_twice_is_refused() {
    let d = deployment("toolsets:\n  - id: issues\n  - id: issues\n");
    assert!(d.validate("toolbox").is_err());
}

#[test]
fn an_empty_access_block_is_refused() {
    let d = deployment("toolsets:\n  - id: issues\n    access: {}\n");
    assert!(d.validate("toolbox").is_err());
    let d = deployment("toolsets:\n  - id: issues\ntools:\n  report_issue:\n    access: {}\n");
    assert!(d.validate("toolbox").is_err());
}

#[test]
fn a_disabled_toolset_is_not_hosted() {
    let d = deployment("toolsets:\n  - id: issues\n  - id: admin\n    enabled: false\n");
    assert!(d.validate("toolbox").is_ok());
    let ids: Vec<&str> = d.enabled_toolsets().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["issues"]);
}

#[test]
fn description_and_removable_round_trip() {
    let d = deployment("description: Toolbox\nremovable: false\n");
    assert_eq!(d.description.as_deref(), Some("Toolbox"));
    assert!(!d.removable);
    assert!(deployment("").removable);
}
