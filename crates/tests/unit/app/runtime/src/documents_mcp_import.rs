use serde_json::json;
use systemprompt_files::documents::agents::{DocumentActor, act_as};
use systemprompt_files::uploads::NewUpload;
use systemprompt_identifiers::{AgentName, ContentHash, TraceId};
use systemprompt_extension::mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{ActorKind, DocumentFormat, content_hash};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::documents::mcp::{
    CreateOutput, DocumentsMcpTools, FolderHandler, FolderInput, ImportHandler, ImportInput,
};
use systemprompt_runtime::documents::refusal::RefusalCode;

use crate::documents_mcp_rig::{Rig, code_of, rig};
use crate::documents_mcp_support::{ctx_for, exec, registered};

const PAGE: &str = "<!doctype html><html><head><style>.k{color:red}</style>\
<script>window.addEventListener('load',()=>{document.body.dataset.ready='1'})</script></head>\
<body><h1 class=\"k\" data-pending=\"x\">Report</h1><p onclick=\"go()\">Body</p></body></html>";

async fn import(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    value: serde_json::Value,
) -> Result<(CreateOutput, String), rmcp::ErrorData> {
    let input: ImportInput = serde_json::from_value(value).expect("import input");
    act_as(
        DocumentActor::agent(AgentName::new("import-agent"), false),
        ImportHandler { tools }.handle(input, ctx, &exec()),
    )
    .await
}

async fn uploaded(
    r: &Rig,
    user: &AuthenticatedUser,
    name: &str,
    mime: &str,
    bytes: &[u8],
) -> ContentHash {
    r.runtime
        .uploads()
        .put_inline(
            &user.id,
            NewUpload {
                name: name.to_owned(),
                mime: mime.to_owned(),
                size: bytes.len() as u64,
                sha256: content_hash(bytes),
            },
            bytes.to_vec(),
            &TraceId::generate(),
        )
        .await
        .expect("upload")
        .blob
}

fn unique(name: &str) -> String {
    format!("{name}-{}", uuid::Uuid::new_v4().simple())
}

fn unique_page() -> String {
    format!("{PAGE}<!-- {} -->", uuid::Uuid::new_v4().simple())
}

#[tokio::test]
async fn an_agent_imports_scripted_html_byte_for_byte_into_a_folder() {
    let r = rig(true).await;
    let user = registered("mcp-import").await;
    let ctx = ctx_for(Some(&user));
    let page = unique_page();
    let blob = uploaded(&r, &user, "page.html", "text/html", page.as_bytes()).await;
    let folder_input: FolderInput = serde_json::from_value(
        json!({"action": "create", "name": format!("Artifacts-{}", uuid::Uuid::new_v4().simple())}),
    )
    .expect("folder input");
    let (folder, _) = FolderHandler { tools: &r.tools }
        .handle(folder_input, &ctx, &exec())
        .await
        .expect("folder");

    let (created, summary) = import(
        &r.tools,
        &ctx,
        json!({"blob_ref": blob, "name": "report.html", "folder_id": folder.folder_id}),
    )
    .await
    .expect("import");
    assert_eq!(created.format, DocumentFormat::Html);
    assert_eq!(created.name, "report.html");
    assert!(!created.written.is_empty(), "{summary}");
    assert!(
        created.written.iter().any(|line| line.ends_with("| Body")),
        "{:?}",
        created.written
    );

    let (revision, bytes) = r
        .tools
        .service()
        .read_bytes(&user, &created.document_id, None, &TraceId::generate())
        .await
        .expect("read back");
    assert_eq!(revision.id, created.revision_id);
    assert_eq!(revision.actor_kind, ActorKind::Agent);
    assert_eq!(bytes, page.as_bytes());

    let document = r
        .tools
        .service()
        .get(&user, &created.document_id, &TraceId::generate())
        .await
        .expect("get")
        .document;
    assert_eq!(document.folder_id.as_ref(), Some(&folder.folder_id));
}

#[tokio::test]
async fn format_overrides_the_extension_and_unknown_extensions_import_as_files() {
    let r = rig(true).await;
    let user = registered("mcp-import-format").await;
    let ctx = ctx_for(Some(&user));
    let page = unique_page();
    let blob = uploaded(&r, &user, "page.html", "text/html", page.as_bytes()).await;
    let (as_html, _) = import(
        &r.tools,
        &ctx,
        json!({"blob_ref": blob, "name": unique("page"), "format": "html"}),
    )
    .await
    .expect("explicit format");
    assert_eq!(as_html.format, DocumentFormat::Html);

    let (as_file, _) = import(
        &r.tools,
        &ctx,
        json!({"blob_ref": blob, "name": format!("{}.weird", unique("page"))}),
    )
    .await
    .expect("file");
    assert_eq!(as_file.format, DocumentFormat::File);
}

#[tokio::test]
async fn another_users_blob_and_an_unknown_blob_are_not_found() {
    let r = rig(true).await;
    let owner = registered("mcp-import-owner").await;
    let other = registered("mcp-import-other").await;
    let blob = uploaded(
        &r,
        &owner,
        "page.html",
        "text/html",
        unique_page().as_bytes(),
    )
    .await;
    let foreign = import(
        &r.tools,
        &ctx_for(Some(&other)),
        json!({"blob_ref": blob, "name": "stolen.html"}),
    )
    .await
    .expect_err("another user's blob");
    assert_eq!(code_of(&foreign), RefusalCode::NotFound);
    let unknown = import(
        &r.tools,
        &ctx_for(Some(&owner)),
        json!({"blob_ref": content_hash(b"never uploaded"), "name": "x.html"}),
    )
    .await
    .expect_err("unknown blob");
    assert_eq!(code_of(&unknown), RefusalCode::NotFound);
}

#[tokio::test]
async fn a_blob_that_did_not_pass_the_formats_check_is_refused() {
    let r = rig(true).await;
    let user = registered("mcp-import-unchecked").await;
    let ctx = ctx_for(Some(&user));
    let bytes = format!("plain words {}", uuid::Uuid::new_v4()).into_bytes();
    let blob = uploaded(&r, &user, "notes.txt", "text/plain", &bytes).await;
    let refused = import(
        &r.tools,
        &ctx,
        json!({"blob_ref": blob, "name": "notes.docx"}),
    )
    .await
    .expect_err("a text blob as docx");
    assert_eq!(code_of(&refused), RefusalCode::InvalidParams);
    assert!(
        refused.message.contains("unchecked_format"),
        "{}",
        refused.message
    );
}

#[tokio::test]
async fn a_server_without_uploads_refuses_the_import() {
    let r = rig(false).await;
    let user = registered("mcp-import-none").await;
    let error = import(
        &r.tools,
        &ctx_for(Some(&user)),
        json!({"blob_ref": content_hash(b"x"), "name": "x.html"}),
    )
    .await
    .expect_err("no uploads");
    assert_eq!(code_of(&error), RefusalCode::InvalidParams);
    assert!(error.message.contains("does not accept uploads"));
}
