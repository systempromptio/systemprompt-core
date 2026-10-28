use std::sync::Arc;
use systemprompt_users::UserRepository;

use systemprompt_governance::authz::{AllowAllHook, SharedAuthzHook};
use systemprompt_identifiers::{
    AgentName, ContextId, DocumentId, DocumentRevisionId, DocumentTemplateId, McpExecutionId,
    McpServerId, SessionId, TraceId, UserId,
};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::documents::EditOp;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::documents::mcp::{
    CreateHandler, CreateInput, DocumentsMcpConfig, DocumentsMcpTools, EditHandler, EditInput,
    ReadHandler,
};
use systemprompt_runtime::documents::views::{ReadInput, ReadOutput, ReadView};
use systemprompt_storage::LocalFileStorage;
use systemprompt_traits::FileStorage;
use tempfile::TempDir;
use uuid::Uuid;

pub(crate) const SERVER: &str = "documents";

pub(crate) struct McpHarness {
    pub(crate) tools: DocumentsMcpTools,
    _storage: TempDir,
}

pub(crate) fn allow() -> SharedAuthzHook {
    Arc::new(AllowAllHook::null())
}

/// The tools driven through their handlers directly, as the person: these
/// tests cover what each tool does, and the agent caps the MCP surface puts
/// around them are covered in the files integration tests.
pub(crate) async fn harness() -> McpHarness {
    let pool = systemprompt_test_fixtures::test_db_pool().await;
    let storage = TempDir::new().expect("tempdir");
    let files: Arc<dyn FileStorage> = Arc::new(LocalFileStorage::new(storage.path().to_path_buf()));
    let runtime = systemprompt_test_fixtures::fixture_documents_runtime(&pool, &files, &allow());
    let tools = DocumentsMcpTools::new(DocumentsMcpConfig {
        service: Arc::clone(runtime.service()),
        uploads: Some(Arc::clone(runtime.uploads())),
        users: Arc::new(UserRepository::new(&pool)),
        tokens: systemprompt_test_fixtures::test_token_authority("documents-test"),
        server_name: McpServerId::new(SERVER),
        api_external_url: "https://docs.example.test/".to_owned(),
    });
    McpHarness {
        tools,
        _storage: storage,
    }
}

pub(crate) fn person(prefix: &str) -> AuthenticatedUser {
    let id = format!("{prefix}-{}", Uuid::new_v4().simple());
    AuthenticatedUser::new(
        UserId::new(id.clone()),
        id.clone(),
        format!("{id}@example.test"),
        vec![Permission::User],
    )
}

pub(crate) fn ctx_for(user: Option<&AuthenticatedUser>) -> RequestContext {
    let actor_id = user.map_or_else(|| UserId::new("anonymous"), |u| u.id.clone());
    let ctx = RequestContext::new(
        SessionId::new(format!("s-{}", Uuid::new_v4().simple())),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("documents-test").expect("agent name"),
        systemprompt_identifiers::Actor::user(actor_id),
    );
    match user {
        Some(user) => ctx.with_user(user.clone()),
        None => ctx,
    }
}

pub(crate) fn exec() -> McpExecutionId {
    McpExecutionId::generate()
}

pub(crate) async fn create_text(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    text: &str,
) -> (DocumentId, DocumentRevisionId) {
    let (created, _) = CreateHandler { tools }
        .handle(
            CreateInput {
                template_id: DocumentTemplateId::new("blank-text"),
                folder_id: None,
                name: format!("notes-{}", Uuid::new_v4().simple()),
                params: Default::default(),
                content: None,
                ops: Vec::new(),
                message: None,
            },
            ctx,
            &exec(),
        )
        .await
        .expect("create");
    let (edited, _) = EditHandler { tools }
        .handle(
            EditInput {
                document_id: created.document_id.clone(),
                base_revision: Some(created.revision_id),
                mode: None,
                op_id: None,
                answering: None,
                allow_detach: false,
                ops: vec![EditOp::SetText {
                    text: text.to_owned(),
                }],
                message: Some("seed".to_owned()),
                verbose: false,
            },
            ctx,
            &exec(),
        )
        .await
        .expect("seed text");
    (created.document_id, edited.revision_id)
}

pub(crate) async fn read_text(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    document: &DocumentId,
) -> ReadOutput {
    ReadHandler { tools }
        .handle(
            ReadInput {
                document_id: document.clone(),
                view: ReadView::Text,
                range: None,
                sheet: None,
                cells: None,
                revision_id: None,
                since_revision: None,
                node_id: None,
                text: None,
                verbose: false,
            },
            ctx,
            &exec(),
        )
        .await
        .expect("read")
        .0
}

pub(crate) fn line_for<'a>(read: &'a ReadOutput, text: &str) -> (&'a str, &'a str) {
    let line = read
        .lines
        .iter()
        .find(|line| line.ends_with(&format!("| {text}")))
        .unwrap_or_else(|| panic!("no line for {text} in {:?}", read.lines));
    let (anchor, _) = line.split_once('|').expect("hashline");
    anchor.rsplit_once(':').expect("node:hash")
}

pub(crate) async fn registered(prefix: &str) -> AuthenticatedUser {
    let user = person(prefix);
    let pool = systemprompt_test_fixtures::test_db_pool().await;
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.id.as_str())
        .bind(&user.email)
        .execute(&*pool.write_pool())
        .await
        .expect("user row");
    user
}
