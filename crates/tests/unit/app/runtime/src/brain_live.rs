use std::sync::Arc;

use serde_json::json;
use systemprompt_extension::DynNotificationSink;
use systemprompt_extension::brain::SourceContent;
use systemprompt_identifiers::{
    BrainFactId, DocumentCommentId, DocumentId, DocumentRevisionId, InstanceId, SessionId, UserId,
};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::brain::FactSource;
use systemprompt_runtime::brain::mcp::{
    BrainMcpTools, BrainRefusalCode, DisputeHandler, ProposeHandler, RecallPreviewHandler,
    SearchHandler, WhyHandler, refusal_code,
};
use systemprompt_runtime::brain::{BrainHandle, DocumentSourceContent};
use systemprompt_runtime::documents::NotificationsUnavailableSink;
use systemprompt_runtime::documents::mcp::{CommentHandler, CommentInput};
use systemprompt_runtime::{BrainCore, BrainInputs, RecallSeams};

use crate::documents_mcp_rig::{Rig, rig};
use crate::documents_mcp_support::{create_text, ctx_for, exec, registered};

fn handle_for(r: &Rig) -> Arc<BrainHandle> {
    let listener_pool = r.pool.write_pool().as_ref().clone();
    let notifications: DynNotificationSink = Arc::new(NotificationsUnavailableSink);
    let profile = BrainProfile {
        enabled: true,
        ..BrainProfile::default()
    };
    let core = BrainCore::new(&BrainInputs {
        database: &r.pool,
        instance_id: &InstanceId::new("brain-live"),
        listener_pool: &listener_pool,
        notifications: &notifications,
        profile: &profile,
    })
    .expect("enabled core");
    core.assemble(&r.runtime, RecallSeams::default())
}

fn code(error: &rmcp::ErrorData) -> BrainRefusalCode {
    refusal_code(error).unwrap_or_else(|| panic!("not a refusal: {error:?}"))
}

fn parse<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).expect("input")
}

#[tokio::test]
async fn an_assembled_handle_exposes_its_parts_and_tools() {
    let r = rig(false).await;
    let handle = handle_for(&r);
    assert!(handle.profile().enabled);
    assert!(handle.sink().is_available());
    assert!(handle.consumer().is_none());
    let (_, recall, profile) = handle.routes();
    assert_eq!(recall.name(), handle.memory_recall().name());
    assert!(Arc::ptr_eq(&profile, handle.profile()));
    let tools = handle.mcp_tools().expect("tools");
    assert!(tools.profile().enabled);
    assert!(format!("{handle:?}").contains("consumer: false"));
}

#[tokio::test]
async fn live_search_scans_the_callers_facts_and_refuses_a_bad_subject() {
    let r = rig(false).await;
    let tools: BrainMcpTools = handle_for(&r).mcp_tools().expect("tools");
    let user = registered("brain-search").await;
    let ctx = ctx_for(Some(&user));
    let (found, summary) = SearchHandler { tools: &tools }
        .handle(
            parse(json!({"query": "  Nothing-Here  ", "kind": "decision",
                         "subject": "repo:core", "scope": "user", "limit": 500})),
            &ctx,
            &exec(),
        )
        .await
        .expect("search");
    assert!(found.facts.is_empty());
    assert_eq!(summary, "0 facts");
    let error = SearchHandler { tools: &tools }
        .handle(
            parse(json!({"query": "x", "subject": "no subject"})),
            &ctx,
            &exec(),
        )
        .await
        .expect_err("subject");
    assert_eq!(code(&error), BrainRefusalCode::InvalidInput);
    let anonymous = SearchHandler { tools: &tools }
        .handle(parse(json!({"query": "x"})), &ctx_for(None), &exec())
        .await
        .expect_err("anonymous");
    assert_eq!(code(&anonymous), BrainRefusalCode::Forbidden);
}

#[tokio::test]
async fn proposing_refuses_a_governing_status_an_unknown_kind_and_a_bad_subject() {
    let r = rig(false).await;
    let tools = handle_for(&r).mcp_tools().expect("tools");
    let user = registered("brain-propose").await;
    let ctx = ctx_for(Some(&user));
    let propose = |value: serde_json::Value| {
        let tools = &tools;
        let ctx = &ctx;
        async move {
            ProposeHandler { tools }
                .handle(parse(value), ctx, &exec())
                .await
                .expect_err("refused")
        }
    };
    let confirmed = propose(json!({"kind": "decision", "subject": "repo:core",
                                   "fields": {}, "status": "confirmed"}))
    .await;
    assert_eq!(code(&confirmed), BrainRefusalCode::AgentForbidden);
    let subject = propose(json!({"kind": "decision", "subject": "nope", "fields": {}})).await;
    assert_eq!(code(&subject), BrainRefusalCode::InvalidInput);
    let kind = format!("undeclared-{}", uuid::Uuid::new_v4().simple());
    let unknown = propose(json!({"kind": kind, "subject": "repo:core", "fields": {}})).await;
    assert_eq!(code(&unknown), BrainRefusalCode::UnknownKind);
}

#[tokio::test]
async fn why_and_dispute_of_an_unknown_fact_are_not_found() {
    let r = rig(false).await;
    let tools = handle_for(&r).mcp_tools().expect("tools");
    let user = registered("brain-why").await;
    let ctx = ctx_for(Some(&user));
    let fact = BrainFactId::generate();
    let why = WhyHandler { tools: &tools }
        .handle(
            parse(json!({"fact_id": fact, "project": "core"})),
            &ctx,
            &exec(),
        )
        .await
        .expect_err("why");
    assert_eq!(code(&why), BrainRefusalCode::NotFound);
    let dispute = DisputeHandler { tools: &tools }
        .handle(
            parse(json!({"fact_id": fact, "reason": "wrong"})),
            &ctx,
            &exec(),
        )
        .await
        .expect_err("dispute");
    assert_eq!(code(&dispute), BrainRefusalCode::NotFound);
    let empty = DisputeHandler { tools: &tools }
        .handle(
            parse(json!({"fact_id": fact, "reason": "   "})),
            &ctx,
            &exec(),
        )
        .await
        .expect_err("empty reason");
    assert_eq!(code(&empty), BrainRefusalCode::InvalidInput);
}

#[tokio::test]
async fn a_recall_preview_with_no_facts_is_skipped() {
    let r = rig(false).await;
    let tools = handle_for(&r).mcp_tools().expect("tools");
    let user = registered("brain-recall").await;
    let (preview, summary) = RecallPreviewHandler { tools: &tools }
        .handle(
            parse(json!({"prompt": "how do we deploy core?", "project": "core"})),
            &ctx_for(Some(&user)),
            &exec(),
        )
        .await
        .expect("preview");
    assert!(!preview.injected);
    assert!(preview.block.is_none());
    assert!(preview.skipped.is_some());
    assert_eq!(preview.tokens, 0);
    assert!(!summary.is_empty());
}

#[tokio::test]
async fn document_source_content_reads_revisions_and_comment_threads() {
    let r = rig(false).await;
    let source = DocumentSourceContent::new(
        Arc::clone(r.runtime.service()),
        Arc::clone(r.runtime.repositories()),
    );
    let user = registered("brain-source").await;
    let ctx = ctx_for(Some(&user));
    let (document, revision) = create_text(&r.tools, &ctx, "alpha\nbeta").await;
    let at = FactSource::Document {
        document_id: document.clone(),
        revision_id: revision,
        span: None,
    };
    assert!(source.handles(&at));
    assert_eq!(source.name(), "documents");
    let text = source.fetch(&at).await.expect("fetch").expect("document");
    assert_eq!(text.text, "alpha\nbeta");
    assert_eq!(text.nodes.len(), 2);
    assert_eq!(
        text.nodes[1].hash8,
        systemprompt_models::documents::hash8("beta")
    );

    let read = crate::documents_mcp_support::read_text(&r.tools, &ctx, &document).await;
    let node = crate::documents_mcp_support::line_for(&read, "alpha")
        .0
        .to_owned();
    let comment = |value: serde_json::Value| {
        let mut input = json!({"document_id": document});
        input
            .as_object_mut()
            .unwrap()
            .extend(value.as_object().unwrap().clone());
        serde_json::from_value::<CommentInput>(input).expect("comment input")
    };
    let (root, _) = CommentHandler { tools: &r.tools }
        .handle(
            comment(json!({"action": "create", "body": "root", "node": node})),
            &ctx,
            &exec(),
        )
        .await
        .expect("root");
    let root_id = root.comments[0].id.clone();
    CommentHandler { tools: &r.tools }
        .handle(
            comment(json!({"action": "create", "body": "reply", "parent_id": root_id})),
            &ctx,
            &exec(),
        )
        .await
        .expect("reply");
    let thread = FactSource::Comment {
        document_id: document.clone(),
        comment_id: root_id,
    };
    let text = source.fetch(&thread).await.expect("fetch").expect("thread");
    assert_eq!(text.text, "root\n\nreply");
    assert!(text.nodes.is_empty());

    let gone = FactSource::Comment {
        document_id: document,
        comment_id: DocumentCommentId::generate(),
    };
    assert!(source.fetch(&gone).await.expect("fetch").is_none());
    let missing = FactSource::Document {
        document_id: DocumentId::generate(),
        revision_id: DocumentRevisionId::generate(),
        span: None,
    };
    assert!(source.fetch(&missing).await.expect("fetch").is_none());
    let session = FactSource::Session {
        session_id: SessionId::generate(),
        owner: UserId::new("someone"),
    };
    assert!(!source.handles(&session));
    assert!(source.fetch(&session).await.expect("fetch").is_none());
}
