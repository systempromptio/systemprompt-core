//! DB-backed tests for `core artifacts show` and `core artifacts list`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_agent::models::a2a::{
    Artifact, ArtifactMetadata, DataPart, FileContent, FilePart, Part, Task, TaskState, TaskStatus,
    TextPart,
};
use systemprompt_agent::models::context::ContextKind;
use systemprompt_agent::repository::content::ArtifactRepository;
use systemprompt_agent::repository::{A2ARepositories, ContextRepository};
use systemprompt_cli::core::artifacts::{list, show};
use systemprompt_cli::{CliConfig, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ArtifactId, ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_test_fixtures::{
    fixture_database_url, fixture_db_pool, seed_user_row, seed_user_session, unique_user_id,
};

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn cfg() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

async fn seed_task(pool: &DbPool) -> (UserId, ContextId, TaskId) {
    let user_id = unique_user_id("cliartifacts");
    let session_id = SessionId::generate();
    let email = format!("{}@cliartifacts.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();
    seed_user_session(pool, &user_id, &session_id)
        .await
        .unwrap();

    let repos = A2ARepositories::new(pool).unwrap();
    let ctx_repo = ContextRepository::new(pool).unwrap();
    let context_id = ctx_repo
        .create_context(
            &user_id,
            Some(&session_id),
            "cli-artifact-context",
            ContextKind::User,
        )
        .await
        .unwrap();

    let task_id = TaskId::generate();
    let task = Task {
        id: task_id.clone(),
        context_id: context_id.clone(),
        status: TaskStatus {
            state: TaskState::Submitted,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        history: None,
        artifacts: None,
        metadata: None,
        created_at: Some(chrono::Utc::now()),
        last_modified: Some(chrono::Utc::now()),
    };
    repos
        .tasks
        .create_task(systemprompt_agent::repository::task::RepoCreateTaskParams {
            task: &task,
            user_id: &user_id,
            session_id: &session_id,
            trace_id: &TraceId::generate(),
            agent_name: "cli-test-agent",
        })
        .await
        .unwrap();

    (user_id, context_id, task_id)
}

async fn seed_artifact(pool: &DbPool, context_id: &ContextId, task_id: &TaskId) -> ArtifactId {
    let artifact_id = ArtifactId::generate();
    let artifact = Artifact {
        id: artifact_id.clone(),
        title: Some("cli artifact".to_owned()),
        description: Some("shown by the CLI".to_owned()),
        parts: vec![
            Part::Text(TextPart {
                text: "short text".to_owned(),
            }),
            Part::Data(DataPart {
                data: serde_json::json!({"answer": 42})
                    .as_object()
                    .unwrap()
                    .clone(),
            }),
            Part::File(FilePart {
                file: FileContent {
                    name: Some("report.txt".to_owned()),
                    mime_type: Some("text/plain".to_owned()),
                    bytes: Some("aGVsbG8=".to_owned()),
                    url: None,
                },
            }),
        ],
        extensions: vec![],
        metadata: ArtifactMetadata::new("text".to_owned(), context_id.clone(), task_id.clone())
            .with_tool_name("cli-tool".to_owned()),
    };
    ArtifactRepository::new(pool)
        .unwrap()
        .create_artifact(task_id, context_id, &artifact)
        .await
        .unwrap();
    artifact_id
}

#[tokio::test]
async fn show_renders_artifact_by_full_id() {
    let pool = pool().await;
    let (_user, context_id, task_id) = seed_task(&pool).await;
    let artifact_id = seed_artifact(&pool, &context_id, &task_id).await;

    let out = show::execute_with_pool(
        show::ShowArgs {
            artifact: artifact_id.as_str().to_owned(),
            full: false,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    let json = serde_json::to_value(out.artifact()).unwrap();
    assert_eq!(json["title"], "Artifact Details");
}

#[tokio::test]
async fn show_renders_text_output_with_full_flag() {
    let pool = pool().await;
    let (_user, context_id, task_id) = seed_task(&pool).await;
    let artifact_id = seed_artifact(&pool, &context_id, &task_id).await;

    show::execute_with_pool(
        show::ShowArgs {
            artifact: artifact_id.as_str().to_owned(),
            full: true,
        },
        &pool,
        &CliConfig::new().with_interactive(false),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn show_resolves_unique_prefix() {
    let pool = pool().await;
    let (_user, context_id, task_id) = seed_task(&pool).await;
    let artifact_id = seed_artifact(&pool, &context_id, &task_id).await;

    let prefix = &artifact_id.as_str()[..artifact_id.as_str().len() - 4];
    show::execute_with_pool(
        show::ShowArgs {
            artifact: prefix.to_owned(),
            full: false,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn show_errors_when_nothing_matches() {
    let pool = pool().await;
    let err = show::execute_with_pool(
        show::ShowArgs {
            artifact: "no-such-artifact-prefix".to_owned(),
            full: false,
        },
        &pool,
        &cfg(),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("No artifact found matching"));
}

#[tokio::test]
async fn list_filters_by_context_and_user() {
    let pool = pool().await;
    let (user_id, context_id, task_id) = seed_task(&pool).await;
    seed_artifact(&pool, &context_id, &task_id).await;

    let by_context = list::execute_with_pool(
        list::ListArgs {
            context: Some(context_id.as_str().to_owned()),
            limit: 10,
        },
        &user_id,
        &pool,
        &cfg(),
    )
    .await
    .unwrap();
    let json = serde_json::to_value(by_context.artifact()).unwrap();
    assert_eq!(json["items"].as_array().unwrap().len(), 1);

    list::execute_with_pool(
        list::ListArgs {
            context: None,
            limit: 10,
        },
        &user_id,
        &pool,
        &CliConfig::new().with_interactive(false),
    )
    .await
    .unwrap();
}
