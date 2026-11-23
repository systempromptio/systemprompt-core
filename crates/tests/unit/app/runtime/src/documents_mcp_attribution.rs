use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;
use systemprompt_agent::models::context::ContextKind;
use systemprompt_agent::repository::ContextRepository;
use systemprompt_agent::services::ContextProviderService;
use systemprompt_ai::{AiService, AiServiceProviders, McpTool, ToolCall};
use systemprompt_documents::mcp::{
    CommentHandler, CommentInput, CreateHandler, CreateInput, DocumentsMcpTools, HistoryHandler,
    HistoryInput,
};
use systemprompt_documents::refusal::RefusalCode;
use systemprompt_extension::mcp::McpToolHandler;
use systemprompt_files::documents::agents::{DocumentActor, act_as};
use systemprompt_files::documents::service::HistoryPage;
use systemprompt_identifiers::{
    AgentName, AiToolCallId, ClientId, ContextId, DocumentId, McpServerId, SessionId, TraceId,
    headers,
};
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::catalog::CatalogFixture;
use systemprompt_test_fixtures::{
    ensure_test_secrets_bootstrap, seed_user_row, seed_user_session, unique_user_id,
};
use systemprompt_traits::{
    AiProviderResult, AiSessionProvider, CreateAiSessionParams, ToolCallRequest, ToolCallResult,
    ToolContent, ToolContext, ToolInventory, ToolProvider, ToolProviderError, ToolProviderResult,
};
use uuid::Uuid;

use crate::documents_mcp_rig::{Rig, code_of, rig};
use crate::documents_mcp_support::{SERVER, create_text, exec, line_for, person, read_text};

const CLIENT: &str = "sp_desktop";

struct NoSessions;

#[async_trait]
impl AiSessionProvider for NoSessions {
    async fn create_session(&self, _params: CreateAiSessionParams<'_>) -> AiProviderResult<()> {
        Ok(())
    }

    async fn increment_ai_usage(
        &self,
        _session_id: &SessionId,
        _tokens: i32,
        _cost_microdollars: i64,
    ) -> AiProviderResult<()> {
        Ok(())
    }

    async fn find_live_session(
        &self,
        _session_id: &SessionId,
    ) -> AiProviderResult<Option<systemprompt_traits::ActiveSession>> {
        Ok(None)
    }
}

type Outcome = Result<DocumentId, RefusalCode>;

struct DocumentsOverAdapter {
    tools: DocumentsMcpTools,
    user: AuthenticatedUser,
    outcomes: Mutex<Vec<Outcome>>,
}

impl DocumentsOverAdapter {
    fn server_side_context(&self, context: &ToolContext) -> RequestContext {
        let session = context
            .session_id
            .clone()
            .expect("the adapter forwards the session");
        let forwarded = context
            .headers
            .get(headers::CONTEXT_ID)
            .filter(|value| !value.is_empty())
            .and_then(|value| ContextId::try_new(value.as_str()).ok())
            .unwrap_or_else(|| ContextId::derived_from_session(&session));
        assert_eq!(
            context.headers.get(headers::USER_ID).map(String::as_str),
            Some(self.user.id.as_str())
        );
        RequestContext::new(
            session,
            context.trace_id.clone().unwrap_or_else(TraceId::generate),
            forwarded,
            AgentName::new("chat-agent"),
            systemprompt_identifiers::Actor::user(self.user.id.clone()),
        )
        .with_user(self.user.clone())
    }
}

#[async_trait]
impl ToolProvider for DocumentsOverAdapter {
    async fn list_tools(
        &self,
        _agent_name: &AgentName,
        _context: &ToolContext,
    ) -> ToolProviderResult<ToolInventory> {
        Ok(ToolInventory::default())
    }

    async fn call_tool(
        &self,
        request: &ToolCallRequest,
        _service_id: &McpServerId,
        context: &ToolContext,
    ) -> ToolProviderResult<ToolCallResult> {
        let ctx = self.server_side_context(context);
        let input: CreateInput = serde_json::from_value(request.arguments.clone())
            .map_err(|error| ToolProviderError::Internal(Box::new(error)))?;
        let outcome = act_as(
            DocumentActor::agent(AgentName::new("chat-agent"), true),
            CreateHandler { tools: &self.tools }.handle(input, &ctx, &exec()),
        )
        .await
        .map(|(created, _)| created.document_id)
        .map_err(|error| code_of(&error));
        self.outcomes.lock().expect("outcomes").push(outcome);
        Ok(ToolCallResult {
            content: vec![ToolContent::text("done")],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    async fn refresh_connections(&self, _agent_name: &AgentName) -> ToolProviderResult<()> {
        Ok(())
    }

    async fn health_check(&self) -> ToolProviderResult<HashMap<String, bool>> {
        Ok(HashMap::new())
    }
}

struct Chat {
    rig: Rig,
    user: AuthenticatedUser,
    session: SessionId,
}

async fn chat_user(prefix: &str) -> Chat {
    let rig = rig(false).await;
    let id = unique_user_id(prefix);
    let email = format!("{}@attribution.invalid", id.as_str());
    seed_user_row(&rig.pool, &id, &email)
        .await
        .expect("user row");
    let session = SessionId::generate();
    seed_user_session(&rig.pool, &id, &session)
        .await
        .expect("session row");
    let user = AuthenticatedUser::new(id, prefix.to_owned(), email, vec![Permission::User]);
    Chat { rig, user, session }
}

fn ai_service(chat: &Chat, provider: Arc<DocumentsOverAdapter>) -> AiService {
    let catalog = Arc::new(
        CatalogFixture::embedded_default()
            .default_model("claude-sonnet-5")
            .build(),
    );
    AiService::new(
        catalog,
        &systemprompt_ai::AiConfig::default(),
        AiServiceProviders {
            tools: provider,
            sessions: Arc::new(NoSessions),
            provider_tokens: Default::default(),
            secrets: ensure_test_secrets_bootstrap(),
            session_ttl_seconds: 3600,
        },
        &systemprompt_ai::repository::AiRequestRepositories::new(&chat.rig.pool),
    )
    .expect("AiService builds")
    .with_context_materializer(Arc::new(ContextProviderService::new(
        ContextRepository::new(&chat.rig.pool),
    )))
}

fn chat_context(chat: &Chat, context: ContextId) -> RequestContext {
    RequestContext::new(
        chat.session.clone(),
        TraceId::generate(),
        context,
        AgentName::new("chat-agent"),
        systemprompt_identifiers::Actor::user(chat.user.id.clone()),
    )
    .with_user(chat.user.clone())
    .with_client_id(ClientId::new(CLIENT))
}

async fn create_over_adapter(chat: &Chat, context: ContextId) -> Outcome {
    let provider = Arc::new(DocumentsOverAdapter {
        tools: chat.rig.tools.clone(),
        user: chat.user.clone(),
        outcomes: Mutex::new(Vec::new()),
    });
    let service = ai_service(chat, Arc::clone(&provider));
    let call = ToolCall {
        ai_tool_call_id: AiToolCallId::new(format!("call_{}", Uuid::new_v4().simple())),
        name: "documents_create".to_owned(),
        arguments: json!({
            "template_id": "blank-text",
            "name": format!("chat-{}", Uuid::new_v4().simple()),
        }),
    };
    let tool = McpTool::new("documents_create", McpServerId::new(SERVER));
    let (_, results) = service
        .execute_tools(vec![call], &[tool], &chat_context(chat, context), None)
        .await;
    assert_eq!(results.len(), 1);
    let mut outcomes = provider.outcomes.lock().expect("outcomes");
    assert_eq!(
        outcomes.len(),
        1,
        "the tool call reached the documents server"
    );
    outcomes.pop().expect("one outcome")
}

async fn context_of(chat: &Chat, document: &DocumentId) -> Option<ContextId> {
    let history = chat
        .rig
        .tools
        .service()
        .history(
            &chat.user,
            document,
            HistoryPage {
                limit: 1,
                before: None,
            },
            &TraceId::generate(),
        )
        .await
        .expect("history");
    history
        .first()
        .and_then(|revision| revision.attribution.context_id.clone())
}

#[tokio::test]
async fn an_ai_turn_in_a_fresh_chat_context_writes_through_the_documents_tools() {
    let chat = chat_user("attr-chat").await;
    let fresh = ContextId::generate();
    let document = create_over_adapter(&chat, fresh.clone())
        .await
        .expect("the adapter's unmaterialised chat context is not refused");
    assert_eq!(context_of(&chat, &document).await, Some(fresh.clone()));
    let owned = chat
        .rig
        .runtime
        .contexts()
        .owned_context(&chat.user.id, &fresh)
        .await
        .expect("directory");
    assert!(
        owned.is_some(),
        "the turn's context is now the user's own row"
    );
}

#[tokio::test]
async fn an_ai_turn_in_another_users_context_is_refused() {
    let chat = chat_user("attr-chat-foreign").await;
    let stranger = chat_user("attr-stranger").await;
    let foreign = ContextRepository::new(&chat.rig.pool)
        .create_context(&stranger.user.id, None, "Not yours", ContextKind::User)
        .await
        .expect("foreign context");
    let refused = create_over_adapter(&chat, foreign)
        .await
        .expect_err("a foreign context is never adopted");
    assert_eq!(refused, RefusalCode::UnknownContext);
}

fn create_input(context: Option<ContextId>) -> CreateInput {
    CreateInput {
        template_id: systemprompt_identifiers::DocumentTemplateId::new("blank-text"),
        folder_id: None,
        name: format!("attributed-{}", Uuid::new_v4().simple()),
        params: Default::default(),
        content: None,
        ops: Vec::new(),
        message: Some("first draft".to_owned()),
        context_id: context,
    }
}

async fn create_as_agent(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    context: Option<ContextId>,
) -> Result<DocumentId, RefusalCode> {
    act_as(
        DocumentActor::agent(AgentName::new("desktop-agent"), true),
        CreateHandler { tools }.handle(create_input(context), ctx, &exec()),
    )
    .await
    .map(|(created, _)| created.document_id)
    .map_err(|error| code_of(&error))
}

#[tokio::test]
async fn a_per_call_context_must_be_owned_and_unknown_and_foreign_ids_refuse_alike() {
    let chat = chat_user("attr-per-call").await;
    let stranger = chat_user("attr-per-call-stranger").await;
    let contexts = ContextRepository::new(&chat.rig.pool);
    let own = contexts
        .create_context(&chat.user.id, None, "Quarterly plan", ContextKind::User)
        .await
        .expect("own context");
    let foreign = contexts
        .create_context(&stranger.user.id, None, "Not yours", ContextKind::User)
        .await
        .expect("foreign context");
    let ctx = chat_context(&chat, ContextId::derived_from_session(&chat.session));
    let tools = &chat.rig.tools;

    let derived = create_as_agent(tools, &ctx, None)
        .await
        .expect("the session's derived context is trusted");
    assert_eq!(
        context_of(&chat, &derived).await,
        Some(ContextId::derived_from_session(&chat.session))
    );

    let named = create_as_agent(tools, &ctx, Some(own.clone()))
        .await
        .expect("an owned context is accepted");
    assert_eq!(context_of(&chat, &named).await, Some(own.clone()));

    let unknown = create_as_agent(tools, &ctx, Some(ContextId::generate())).await;
    let forged = create_as_agent(tools, &ctx, Some(foreign)).await;
    assert_eq!(unknown, Err(RefusalCode::UnknownContext));
    assert_eq!(forged, Err(RefusalCode::UnknownContext));

    let header_forged = chat_context(&chat, ContextId::generate());
    assert_eq!(
        create_as_agent(tools, &header_forged, None).await,
        Err(RefusalCode::UnknownContext)
    );
}

#[tokio::test]
async fn revisions_and_comments_record_the_agent_client_and_named_context() {
    let chat = chat_user("attr-record").await;
    let own = ContextRepository::new(&chat.rig.pool)
        .create_context(&chat.user.id, None, "Board memo", ContextKind::User)
        .await
        .expect("own context");
    let ctx = chat_context(&chat, ContextId::derived_from_session(&chat.session));
    let tools = &chat.rig.tools;
    let document = create_as_agent(tools, &ctx, Some(own.clone()))
        .await
        .expect("create");

    let (history, _) = HistoryHandler { tools }
        .handle(
            HistoryInput {
                document_id: document.clone(),
                limit: None,
                verbose: true,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("history");
    let first = &history.revisions[0];
    assert_eq!(first.message.as_deref(), Some("first draft"));
    let attribution = first.attribution.as_ref().expect("verbose attribution");
    assert_eq!(attribution.context_id.as_ref(), Some(&own));
    assert_eq!(attribution.context_name.as_deref(), Some("Board memo"));
    assert_eq!(
        attribution.agent_name.as_ref().map(AgentName::as_str),
        Some(CLIENT)
    );
    assert_eq!(
        attribution.client_id.as_ref().map(ClientId::as_str),
        Some(CLIENT)
    );

    let (commented, _) = create_text(tools, &ctx, "alpha").await;
    let read = read_text(tools, &ctx, &commented).await;
    let node = line_for(&read, "alpha").0.to_owned();
    let comment: CommentInput = serde_json::from_value(json!({
        "action": "create",
        "document_id": commented,
        "node": node,
        "body": "Please check the totals",
        "context_id": own,
    }))
    .expect("comment input");
    act_as(
        DocumentActor::agent(AgentName::new("desktop-agent"), true),
        CommentHandler { tools }.handle(comment, &ctx, &exec()),
    )
    .await
    .expect("comment");
    let list: CommentInput = serde_json::from_value(json!({
        "action": "list",
        "document_id": commented,
    }))
    .expect("list input");
    let (listed, _) = CommentHandler { tools }
        .handle(list, &ctx, &exec())
        .await
        .expect("list");
    let item = &listed.comments[0];
    assert_eq!(item.context_id.as_ref(), Some(&own));
    assert_eq!(item.context_name.as_deref(), Some("Board memo"));
    assert_eq!(item.via_agent.as_ref().map(AgentName::as_str), Some(CLIENT));
    assert_eq!(item.client_id.as_ref().map(ClientId::as_str), Some(CLIENT));

    let stranger = person("attr-record-viewer");
    let names = chat
        .rig
        .runtime
        .contexts()
        .owned_names(&stranger.id, std::slice::from_ref(&own))
        .await
        .expect("names");
    assert!(names.is_empty(), "a context's name is its owner's alone");
}
