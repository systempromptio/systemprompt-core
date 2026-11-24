use serde_json::json;
use systemprompt_documents::mcp::{
    DocumentsMcpTools, FolderHandler, FolderInput, FolderItem, ListHandler, ListInput,
};
use systemprompt_documents::refusal::RefusalCode;
use systemprompt_extension::mcp::McpToolHandler;
use systemprompt_files::documents::agents::{AgentCap, DocumentActor, act_as};
use systemprompt_files::documents::service::AgentCapTarget;
use systemprompt_identifiers::{AgentName, DocumentFolderId, TraceId};
use systemprompt_models::execution::context::RequestContext;

use crate::documents_mcp_rig::{code_of, rig};
use crate::documents_mcp_support::{ctx_for, exec, registered};

async fn folder(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    value: serde_json::Value,
) -> Result<(FolderItem, String), rmcp::ErrorData> {
    let input: FolderInput = serde_json::from_value(value).expect("folder input");
    FolderHandler { tools }.handle(input, ctx, &exec()).await
}

async fn as_agent<F: std::future::Future>(work: F) -> F::Output {
    act_as(
        DocumentActor::agent(AgentName::new("folder-agent"), false),
        work,
    )
    .await
}

fn unique(name: &str) -> String {
    format!("{name}-{}", uuid::Uuid::new_v4().simple())
}

async fn subfolders(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    parent: Option<&DocumentFolderId>,
) -> Vec<FolderItem> {
    let input: ListInput =
        serde_json::from_value(json!({ "folder_id": parent })).expect("list input");
    ListHandler { tools }
        .handle(input, ctx, &exec())
        .await
        .expect("list")
        .0
        .folders
}

#[tokio::test]
async fn an_agent_creates_renames_and_nests_folders_it_lists() {
    let r = rig(true).await;
    let user = registered("mcp-folder").await;
    let ctx = ctx_for(Some(&user));
    let name = unique("Artifacts");
    let (root, summary) = as_agent(folder(
        &r.tools,
        &ctx,
        json!({"action": "create", "name": name}),
    ))
    .await
    .expect("create at the root");
    assert_eq!(root.name, name);
    assert!(root.parent_id.is_none());
    assert!(summary.contains(&name), "{summary}");

    let (child, _) = as_agent(folder(
        &r.tools,
        &ctx,
        json!({"action": "create", "name": "project", "parent_id": root.folder_id}),
    ))
    .await
    .expect("create nested");
    assert_eq!(child.parent_id.as_ref(), Some(&root.folder_id));

    let (renamed, _) = as_agent(folder(
        &r.tools,
        &ctx,
        json!({"action": "update", "folder_id": child.folder_id, "name": "renamed"}),
    ))
    .await
    .expect("rename");
    assert_eq!(renamed.name, "renamed");
    assert_eq!(renamed.parent_id.as_ref(), Some(&root.folder_id));

    let listed = subfolders(&r.tools, &ctx, Some(&root.folder_id)).await;
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].folder_id, child.folder_id);
    assert_eq!(listed[0].name, "renamed");
}

#[tokio::test]
async fn a_folder_name_a_sibling_holds_is_refused_as_name_taken() {
    let r = rig(true).await;
    let user = registered("mcp-folder-taken").await;
    let ctx = ctx_for(Some(&user));
    let name = unique("Taken");
    let (parent, _) = as_agent(folder(
        &r.tools,
        &ctx,
        json!({"action": "create", "name": unique("parent")}),
    ))
    .await
    .expect("parent");
    as_agent(folder(
        &r.tools,
        &ctx,
        json!({"action": "create", "name": name, "parent_id": parent.folder_id}),
    ))
    .await
    .expect("first");
    let again = as_agent(folder(
        &r.tools,
        &ctx,
        json!({"action": "create", "name": name, "parent_id": parent.folder_id}),
    ))
    .await
    .expect_err("same name, same parent");
    assert_eq!(code_of(&again), RefusalCode::NameTaken, "{}", again.message);
    assert!(again.message.contains("folder"), "{}", again.message);

    let (sibling, _) = as_agent(folder(
        &r.tools,
        &ctx,
        json!({"action": "create", "name": "other", "parent_id": parent.folder_id}),
    ))
    .await
    .expect("sibling");
    let renamed = as_agent(folder(
        &r.tools,
        &ctx,
        json!({"action": "update", "folder_id": sibling.folder_id, "name": name}),
    ))
    .await
    .expect_err("rename onto a sibling's name");
    assert_eq!(
        code_of(&renamed),
        RefusalCode::NameTaken,
        "{}",
        renamed.message
    );

    let (elsewhere, _) = folder(&r.tools, &ctx, json!({"action": "create", "name": name}))
        .await
        .expect("same name at the root");
    let moved = folder(
        &r.tools,
        &ctx,
        json!({"action": "update", "folder_id": elsewhere.folder_id, "parent_id": parent.folder_id}),
    )
    .await
    .expect_err("move beside a folder of the same name");
    assert_eq!(code_of(&moved), RefusalCode::NameTaken, "{}", moved.message);
}

#[tokio::test]
async fn the_person_moves_a_folder_and_an_agent_is_refused_the_move() {
    let r = rig(true).await;
    let user = registered("mcp-folder-move").await;
    let ctx = ctx_for(Some(&user));
    let (a, _) = folder(
        &r.tools,
        &ctx,
        json!({"action": "create", "name": unique("a")}),
    )
    .await
    .expect("a");
    let (b, _) = folder(
        &r.tools,
        &ctx,
        json!({"action": "create", "name": unique("b")}),
    )
    .await
    .expect("b");

    let refused = as_agent(folder(
        &r.tools,
        &ctx,
        json!({"action": "update", "folder_id": b.folder_id, "parent_id": a.folder_id}),
    ))
    .await
    .expect_err("an agent moving a folder");
    assert_eq!(code_of(&refused), RefusalCode::AgentForbidden);
    assert!(
        subfolders(&r.tools, &ctx, Some(&a.folder_id))
            .await
            .is_empty()
    );

    let (moved, _) = folder(
        &r.tools,
        &ctx,
        json!({"action": "update", "folder_id": b.folder_id, "parent_id": a.folder_id}),
    )
    .await
    .expect("the person moves it");
    assert_eq!(moved.parent_id.as_ref(), Some(&a.folder_id));

    let (back, _) = folder(
        &r.tools,
        &ctx,
        json!({"action": "update", "folder_id": b.folder_id, "parent_id": null}),
    )
    .await
    .expect("back to the root");
    assert!(back.parent_id.is_none());
}

#[tokio::test]
async fn an_agent_capped_at_suggest_may_not_create_a_folder_there() {
    let r = rig(true).await;
    let user = registered("mcp-folder-suggest").await;
    let ctx = ctx_for(Some(&user));
    let (parent, _) = folder(
        &r.tools,
        &ctx,
        json!({"action": "create", "name": unique("capped")}),
    )
    .await
    .expect("parent");
    r.tools
        .service()
        .set_agent_cap(
            &user,
            AgentCapTarget::Folder(parent.folder_id.clone()),
            Some(AgentCap::Suggest),
            &TraceId::generate(),
        )
        .await
        .expect("cap the folder");
    let refused = as_agent(folder(
        &r.tools,
        &ctx,
        json!({"action": "create", "name": "inside", "parent_id": parent.folder_id}),
    ))
    .await
    .expect_err("suggest refuses a create");
    assert_eq!(code_of(&refused), RefusalCode::AgentSuggestOnly);
}

#[tokio::test]
async fn malformed_folder_calls_are_refused() {
    let r = rig(true).await;
    let user = registered("mcp-folder-bad").await;
    let ctx = ctx_for(Some(&user));
    let (existing, _) = folder(
        &r.tools,
        &ctx,
        json!({"action": "create", "name": unique("x")}),
    )
    .await
    .expect("existing");
    let cases = [
        (json!({"action": "create"}), "name"),
        (
            json!({"action": "create", "name": "y", "folder_id": existing.folder_id}),
            "folder_id",
        ),
        (json!({"action": "update", "name": "y"}), "folder_id"),
        (
            json!({"action": "update", "folder_id": existing.folder_id}),
            "new name",
        ),
    ];
    for (value, needle) in cases {
        let error = folder(&r.tools, &ctx, value.clone())
            .await
            .expect_err("malformed");
        assert_eq!(code_of(&error), RefusalCode::InvalidParams, "{value}");
        assert!(error.message.contains(needle), "{}", error.message);
    }
    let unknown = folder(
        &r.tools,
        &ctx,
        json!({"action": "update", "folder_id": DocumentFolderId::generate(), "name": "z"}),
    )
    .await
    .expect_err("unknown folder");
    assert_eq!(code_of(&unknown), RefusalCode::NotFound);
    let anonymous = folder(
        &r.tools,
        &ctx_for(None),
        json!({"action": "create", "name": "a"}),
    )
    .await
    .expect_err("anonymous");
    assert_eq!(code_of(&anonymous), RefusalCode::Forbidden);
}
