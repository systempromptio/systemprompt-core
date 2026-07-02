// ContextService::load_conversation_history over persisted tasks: text and
// file parts (image and text attachments decoded, unsupported and byteless
// files dropped), role mapping, empty-message skipping, and artifact
// serialization including the long-description truncation.

use base64::Engine;
use systemprompt_agent::models::a2a::{
    Artifact, ArtifactMetadata, DataPart, FileContent, FilePart, Message, MessageRole, Part,
    TaskState, TextPart,
};
use systemprompt_agent::services::ContextService;
use systemprompt_agent::services::a2a_server::processing::persistence_service::{
    PersistCompletedTaskServiceParams, PersistenceService,
};
use systemprompt_identifiers::{ArtifactId, ContextId, MessageId, TaskId};

use super::a2a_server::a2a_helpers::request_context;
use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn file_part(name: &str, mime: Option<&str>, bytes: Option<String>) -> Part {
    Part::File(FilePart {
        file: FileContent {
            name: Some(name.to_owned()),
            mime_type: mime.map(str::to_owned),
            bytes,
            url: None,
        },
    })
}

fn message_with_parts(
    ctx: &ContextId,
    task_id: &TaskId,
    role: MessageRole,
    parts: Vec<Part>,
) -> Message {
    Message {
        role,
        parts,
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: ctx.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn artifact(ctx: &ContextId, task_id: &TaskId, description: Option<String>) -> Artifact {
    Artifact {
        id: ArtifactId::generate(),
        title: Some("report".to_owned()),
        description,
        parts: vec![Part::Text(TextPart {
            text: "artifact body".to_owned(),
        })],
        extensions: vec![serde_json::json!(
            systemprompt_models::a2a::ARTIFACT_RENDERING_URI
        )],
        metadata: ArtifactMetadata {
            artifact_type: "document".to_owned(),
            context_id: ctx.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            task_id: task_id.clone(),
            rendering_hints: None,
            source: None,
            mcp_execution_id: None,
            mcp_schema: None,
            is_internal: None,
            fingerprint: None,
            tool_name: None,
            execution_index: None,
            skill_id: None,
            skill_name: None,
        },
    }
}

#[tokio::test]
async fn history_decodes_parts_and_serializes_artifacts() {
    let Some(pool) = try_pool().await else {
        return;
    };
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let user_msg = message_with_parts(
        &ctx,
        &task_id,
        MessageRole::User,
        vec![
            Part::Text(TextPart {
                text: "look at this".to_owned(),
            }),
            file_part("pic.png", Some("image/png"), Some(b64(b"\x89PNG"))),
            file_part("notes.txt", Some("text/plain"), Some(b64(b"plain notes"))),
            file_part(
                "bad.txt",
                Some("text/plain"),
                Some("!!!not-base64!!!".to_owned()),
            ),
            file_part(
                "mystery.bin",
                Some("application/x-unknown"),
                Some(b64(b"x")),
            ),
            file_part("nomime.bin", None, Some(b64(b"x"))),
            Part::Data(DataPart {
                data: serde_json::Map::from_iter([(
                    "k".to_owned(),
                    serde_json::Value::String("v".to_owned()),
                )]),
            }),
        ],
    );
    let agent_msg = message_with_parts(
        &ctx,
        &task_id,
        MessageRole::Agent,
        vec![Part::Text(TextPart {
            text: "the answer".to_owned(),
        })],
    );

    let mut task = PersistenceService::build_initial_task(task_id.clone(), ctx.clone(), "hist");
    task.status.state = TaskState::Completed;
    task.status.message = Some(agent_msg.clone());
    task.artifacts = Some(vec![
        artifact(&ctx, &task_id, Some("d".repeat(400))),
        artifact(&ctx, &task_id, Some("short".to_owned())),
        artifact(&ctx, &task_id, None),
    ]);

    let service = PersistenceService::new(pool.clone());
    service
        .persist_completed_task(PersistCompletedTaskServiceParams {
            task: &task,
            user_message: &user_msg,
            agent_message: &agent_msg,
            context: &request_context(&ctx, &session, &user, "hist"),
            artifacts_already_published: false,
        })
        .await
        .expect("persist");

    let history = ContextService::new(&pool)
        .expect("service")
        .load_conversation_history(&ctx)
        .await
        .expect("history");

    let user_entry = history
        .iter()
        .find(|m| m.content == "look at this")
        .expect("user message present");
    assert_eq!(user_entry.role, systemprompt_models::MessageRole::User);
    assert!(
        user_entry.parts.len() >= 3,
        "text + image + decoded text file survive, got {}",
        user_entry.parts.len()
    );

    assert!(history.iter().any(|m| m.content == "the answer"));

    let artifact_entries: Vec<_> = history
        .iter()
        .filter(|m| m.content.starts_with("[Artifact: report"))
        .collect();
    assert_eq!(artifact_entries.len(), 3);
    assert!(artifact_entries.iter().any(|m| m.content.contains("...")));
    assert!(artifact_entries.iter().any(|m| m.content.contains("short")));
}

#[tokio::test]
async fn history_for_unknown_context_is_empty() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let history = ContextService::new(&pool)
        .expect("service")
        .load_conversation_history(&ContextId::generate())
        .await
        .expect("history");
    assert!(history.is_empty());
}
