#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use rmcp::model::{CallToolResult, ContentBlock, MetaObject, Resource, ResourceContents};
use serde_json::json;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_mcp::{
    ArtifactIngest, IngestRequest, from_canonical_tool_result, from_hook_response, from_wire_value,
};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::EXECUTION_META_KEY;
use systemprompt_models::auth::UserType;
use systemprompt_models::mcp::ExecutionSource;
use systemprompt_models::wire::canonical::{CanonicalContent, ImageSource};

fn context(session: &str) -> RequestContext {
    RequestContext::new(
        SessionId::new(session),
        TraceId::new(session),
        ContextId::generate(),
        AgentName::try_new("artifact-mixed").unwrap(),
    )
    .with_actor(Actor::user(UserId::new(format!("user-{session}"))))
    .with_user_type(UserType::User)
}

fn request(result: CallToolResult, session: &str) -> IngestRequest {
    IngestRequest {
        result,
        tool_name: "mixed_result".to_owned(),
        server_name: Some("fixture-server".to_owned()),
        ai_tool_call_id: None,
        mcp_execution_id: None,
        ctx: context(session),
        skill: None,
        source: ExecutionSource::Gateway,
        started_at: None,
        input: Some(json!({"query": "fixture"})),
    }
}

async fn ingest() -> ArtifactIngest {
    let url = systemprompt_test_fixtures::fixture_database_url().expect("MCP fixture database URL");
    let db = systemprompt_test_fixtures::fixture_db_pool(&url)
        .await
        .expect("MCP fixture pool");
    ArtifactIngest::from_db(&db, None).expect("artifact ingest")
}

#[test]
fn hook_and_canonical_normalization_preserve_wire_semantics() {
    assert!(from_wire_value(&json!({"content": "not-an-array"})).is_none());
    assert!(
        from_hook_response(&serde_json::Value::Null)
            .content
            .is_empty()
    );
    let strings = from_hook_response(&json!(["first", "second"]));
    assert_eq!(strings.content.len(), 2);
    assert!(matches!(&strings.content[0], ContentBlock::Text(value) if value.text == "first"));
    let mixed = from_hook_response(&json!(["first", 2]));
    assert!(mixed.content.is_empty());
    assert_eq!(mixed.structured_content, Some(json!(["first", 2])));

    let content = vec![
        CanonicalContent::text("plain"),
        CanonicalContent::image(ImageSource::Base64 {
            media_type: "image/png".to_owned(),
            data: "aGVsbG8=".to_owned(),
            detail: None,
        }),
        CanonicalContent::image(ImageSource::Url {
            url: "https://example.invalid/image.png".to_owned(),
            detail: None,
        }),
        CanonicalContent::ToolUse {
            id: "call-1".to_owned(),
            name: "ignored".to_owned(),
            input: json!({}),
            signature: None,
            cache_control: None,
        },
        CanonicalContent::Thinking {
            text: "ignored".to_owned(),
            signature: None,
            id: None,
            encrypted_content: None,
        },
    ];
    let mut meta = json!({});
    meta[EXECUTION_META_KEY] = json!({"artifact_id": "artifact-wire"});
    let result =
        from_canonical_tool_result(&content, Some(&json!({"answer": 42})), Some(&meta), true);
    assert_eq!(result.content.len(), 3);
    assert_eq!(result.is_error, Some(true));
    assert_eq!(result.structured_content, Some(json!({"answer": 42})));
    assert_eq!(
        result.meta.unwrap().0[EXECUTION_META_KEY]["artifact_id"],
        "artifact-wire"
    );
}

#[tokio::test]
async fn mixed_binary_and_resource_blocks_persist_as_bounded_metadata() {
    let ingest = ingest().await;
    let session = format!("mixed-{}", uuid::Uuid::new_v4().simple());
    let result = CallToolResult::success(vec![
        ContentBlock::text("visible text"),
        ContentBlock::image("aGVsbG8=", "image/png"),
        ContentBlock::audio("YXVkaW8=", "audio/wav"),
        ContentBlock::resource(ResourceContents::TextResourceContents {
            uri: "ui://fixture/text".to_owned(),
            mime_type: Some("text/html".to_owned()),
            text: "<main>safe</main>".to_owned(),
            meta: None,
        }),
        ContentBlock::resource(ResourceContents::BlobResourceContents {
            uri: "file://fixture/blob".to_owned(),
            mime_type: Some("application/octet-stream".to_owned()),
            blob: "AAECAw==".to_owned(),
            meta: None,
        }),
        ContentBlock::resource_link(
            Resource::new(
                "https://example.invalid/report".to_owned(),
                "report".to_owned(),
            )
            .with_mime_type("application/json".to_owned()),
        ),
    ]);

    let outcome = ingest
        .ingest(request(result, &session))
        .await
        .expect("mixed result ingest");
    assert!(!outcome.is_structured);
    let blocks = outcome.stored_body["blocks"].as_array().expect("blocks");
    assert_eq!(blocks.len(), 6);
    assert_eq!(blocks[0], json!({"type": "text", "text": "visible text"}));
    assert_eq!(blocks[1]["type"], "image");
    assert_eq!(blocks[1]["mime_type"], "image/png");
    assert_eq!(blocks[1]["byte_len"], 8);
    assert!(
        blocks[1]["sha256"]
            .as_str()
            .is_some_and(|value| value.len() == 64)
    );
    assert_eq!(blocks[2]["type"], "audio");
    assert_eq!(blocks[3]["uri"], "ui://fixture/text");
    assert_eq!(blocks[3]["text"], "<main>safe</main>");
    assert_eq!(blocks[4]["blob_byte_len"], 8);
    assert!(blocks[4]["blob_sha256"].as_str().is_some());
    assert_eq!(blocks[5]["type"], "resource_link");
    assert_eq!(blocks[5]["name"], "report");

    let stored = ingest
        .artifacts()
        .find_by_id(&outcome.artifact_id)
        .await
        .unwrap()
        .unwrap();
    assert!(stored.has_ui_resource);
    assert_eq!(stored.artifact_type, "tool_result");
}

#[tokio::test]
async fn malformed_known_typed_body_retains_declared_schema_and_metadata_identity() {
    let ingest = ingest().await;
    let session = format!("typed-{}", uuid::Uuid::new_v4().simple());
    let mut result = CallToolResult::success(vec![ContentBlock::text("fallback")]);
    result.structured_content = Some(json!({
        "x-artifact-type": "text",
        "title": "Declared text",
        "unexpected": {"kept": true}
    }));
    let mut meta = serde_json::Map::new();
    meta.insert(
        EXECUTION_META_KEY.to_owned(),
        json!({"artifact_id": format!("typed-{}", uuid::Uuid::new_v4().simple()), "mcp_execution_id": ""}),
    );
    result.meta = Some(MetaObject(meta));

    let outcome = ingest
        .ingest(request(result, &session))
        .await
        .expect("declared body ingest");
    assert!(outcome.is_structured);
    assert_eq!(outcome.stored_body["x-artifact-type"], "text");
    assert_eq!(outcome.stored_body["title"], "Declared text");
    assert_eq!(outcome.stored_body["unexpected"]["kept"], true);
    let stored = ingest
        .artifacts()
        .find_by_id(&outcome.artifact_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.artifact_type, "text");
    assert_eq!(stored.title.as_deref(), Some("Declared text"));
    assert!(stored.is_structured);
}
