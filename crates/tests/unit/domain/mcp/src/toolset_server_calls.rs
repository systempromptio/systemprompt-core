use rmcp::model::{CallToolRequestParams, ReadResourceRequestParams, ResourceContents};

use crate::toolset_server_gate::connect;
use crate::toolset_server_live::{admin_caller, allow, deny, fixture, user_caller};

fn names(tools: &[rmcp::model::Tool]) -> Vec<String> {
    tools.iter().map(|t| t.name.to_string()).collect()
}

fn call(name: &'static str) -> CallToolRequestParams {
    CallToolRequestParams::new(name)
}

#[tokio::test]
async fn a_missing_bearer_is_rejected_and_audited() {
    let fx = fixture(allow(), None, false).await;
    let client = connect(&fx.server, Vec::new()).await;
    let err = client.list_tools(None).await.expect_err("no bearer");
    assert!(err.to_string().contains("Authentication required"), "{err}");
    assert_eq!(fx.events(), ["tools/list:rejected:false"]);
}

fn forged_identity_headers(bearer: String) -> Vec<(String, String)> {
    vec![
        ("x-proxy-verified".to_owned(), "true".to_owned()),
        ("x-user-id".to_owned(), "forged-admin".to_owned()),
        ("x-user-permissions".to_owned(), "admin".to_owned()),
        ("x-user-roles".to_owned(), "admin".to_owned()),
        ("x-user-type".to_owned(), "admin".to_owned()),
        ("authorization".to_owned(), bearer),
    ]
}

#[tokio::test]
async fn forged_identity_headers_without_a_hop_token_are_rejected() {
    let fx = fixture(allow(), None, false).await;
    let client = connect(
        &fx.server,
        forged_identity_headers("Bearer forged".to_owned()),
    )
    .await;
    let err = client.list_tools(None).await.expect_err("forged headers");
    assert!(err.to_string().contains("Authentication required"), "{err}");
    assert_eq!(fx.events(), ["tools/list:rejected:false"]);
}

#[tokio::test]
async fn a_valid_user_bearer_without_a_hop_token_is_not_an_identity() {
    let fx = fixture(allow(), None, false).await;
    let bearer = admin_caller()
        .into_iter()
        .find(|(name, _)| name == "authorization")
        .map(|(_, value)| value)
        .expect("bearer");
    let client = connect(&fx.server, forged_identity_headers(bearer)).await;
    let err = client.list_tools(None).await.expect_err("bearer alone");
    assert!(err.to_string().contains("Authentication required"), "{err}");
}

#[tokio::test]
async fn a_plain_user_lists_only_server_scoped_tools() {
    let fx = fixture(allow(), None, false).await;
    let client = connect(&fx.server, user_caller()).await;
    let listed = client.list_tools(None).await.expect("list");
    assert_eq!(names(&listed.tools), ["issues_list", "report_issue"]);
    assert_eq!(fx.events(), ["tools/list:authenticated:true"]);
}

#[tokio::test]
async fn an_admin_in_the_group_lists_every_tool() {
    let fx = fixture(allow(), Some(Ok(vec!["platform".to_owned()])), false).await;
    let client = connect(&fx.server, admin_caller()).await;
    let listed = client.list_tools(None).await.expect("list");
    assert_eq!(
        names(&listed.tools),
        ["grant_role", "issues_list", "report_issue", "users_list"]
    );
}

#[tokio::test]
async fn a_failed_group_lookup_hides_group_tools() {
    let fx = fixture(allow(), Some(Err(())), false).await;
    let client = connect(&fx.server, admin_caller()).await;
    let listed = client.list_tools(None).await.expect("list");
    assert_eq!(
        names(&listed.tools),
        ["issues_list", "report_issue", "users_list"]
    );
}

#[tokio::test]
async fn a_permitted_call_runs_the_owning_toolset_and_is_audited() {
    let fx = fixture(allow(), None, false).await;
    let client = connect(&fx.server, admin_caller()).await;
    let result = client.call_tool(call("users_list")).await.expect("call");
    let body = result.structured_content.expect("structured");
    assert_eq!(body["toolset"], "admin");
    assert_eq!(body["tool"], "users_list");
    assert_eq!(body["user"], "host-admin@test.invalid");
    assert_eq!(
        fx.events(),
        ["users_list:authenticated:true", "users_list:used:true"]
    );
}

#[tokio::test]
async fn a_call_outside_the_callers_access_is_forbidden() {
    let fx = fixture(allow(), None, false).await;
    let client = connect(&fx.server, user_caller()).await;
    let err = client
        .call_tool(call("users_list"))
        .await
        .expect_err("forbidden");
    assert!(err.to_string().contains("Forbidden"), "{err}");
    assert_eq!(
        fx.events(),
        ["users_list:authenticated:true", "users_list:forbidden:true"]
    );
}

#[tokio::test]
async fn a_group_tool_without_group_membership_is_forbidden() {
    let fx = fixture(allow(), Some(Ok(vec!["other".to_owned()])), false).await;
    let client = connect(&fx.server, admin_caller()).await;
    let err = client
        .call_tool(call("grant_role"))
        .await
        .expect_err("forbidden");
    assert!(err.to_string().contains("not granted"), "{err}");
}

#[tokio::test]
async fn a_group_member_may_call_the_group_tool() {
    let fx = fixture(allow(), Some(Ok(vec!["platform".to_owned()])), false).await;
    let client = connect(&fx.server, admin_caller()).await;
    let result = client.call_tool(call("grant_role")).await.expect("call");
    assert_eq!(result.structured_content.expect("body")["toolset"], "admin");
}

#[tokio::test]
async fn an_unknown_tool_is_refused_naming_the_hosted_tools() {
    let fx = fixture(allow(), None, false).await;
    let client = connect(&fx.server, admin_caller()).await;
    let err = client.call_tool(call("nope")).await.expect_err("unknown");
    let text = err.to_string();
    assert!(text.contains("Unknown tool: 'nope'"), "{text}");
    assert!(text.contains("report_issue"), "{text}");
}

#[tokio::test]
async fn a_deny_hook_refuses_the_call_before_the_toolset_runs() {
    let fx = fixture(deny(), None, false).await;
    let client = connect(&fx.server, admin_caller()).await;
    let err = client
        .call_tool(call("issues_list"))
        .await
        .expect_err("denied");
    assert!(err.to_string().contains("authz denied"), "{err}");
    assert_eq!(fx.events(), ["issues_list:rejected:false"]);
}

#[tokio::test]
async fn resources_are_listed_and_read_from_the_serving_toolset() {
    let fx = fixture(allow(), None, true).await;
    let client = connect(&fx.server, user_caller()).await;
    let listed = client.list_resources(None).await.expect("list");
    assert_eq!(listed.resources.len(), 1);
    assert_eq!(listed.resources[0].uri, "issues://open");
    let read = client
        .read_resource(ReadResourceRequestParams::new("issues://open"))
        .await
        .expect("read");
    assert!(matches!(
        &read.contents[0],
        ResourceContents::TextResourceContents { text, .. } if text == "none open"
    ));
    let err = client
        .read_resource(ReadResourceRequestParams::new("issues://closed"))
        .await
        .expect_err("unknown resource");
    assert!(err.to_string().contains("Unknown resource"), "{err}");
}

#[tokio::test]
async fn resource_reads_require_authentication() {
    let fx = fixture(allow(), None, true).await;
    let client = connect(&fx.server, Vec::new()).await;
    let err = client.list_resources(None).await.expect_err("no bearer");
    assert!(err.to_string().contains("Authentication required"), "{err}");
    assert_eq!(fx.events(), ["resources/list:rejected:false"]);
}
