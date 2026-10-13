use bytes::Bytes;
use futures::stream;
use systemprompt_cli::commands::admin::agents::message_streaming::collect_stream;
use systemprompt_models::a2a::TaskState;

const TASK_ID: &str = "00000000-0000-4000-8000-0000000000a1";
const CONTEXT_ID: &str = "00000000-0000-4000-8000-0000000000c1";

fn sse(frames: &[serde_json::Value]) -> Vec<Result<Bytes, std::io::Error>> {
    frames
        .iter()
        .map(|frame| Ok(Bytes::from(format!("data: {frame}\n\n"))))
        .collect()
}

fn status_frame(state: &str, text: Option<&str>) -> serde_json::Value {
    let mut status = serde_json::json!({ "state": state });
    if let Some(text) = text {
        status["message"] = serde_json::json!({
            "role": "ROLE_AGENT",
            "parts": [{ "text": text }],
            "messageId": "00000000-0000-4000-8000-0000000000d1",
            "contextId": CONTEXT_ID
        });
    }
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "1",
        "result": {
            "statusUpdate": {
                "taskId": TASK_ID,
                "contextId": CONTEXT_ID,
                "status": status
            }
        }
    })
}

#[tokio::test]
async fn status_updates_then_close_yield_the_final_task() {
    let frames = sse(&[
        status_frame("TASK_STATE_WORKING", Some("hello ")),
        status_frame("TASK_STATE_COMPLETED", Some("world")),
    ]);
    let mut printed = String::new();
    let outcome = collect_stream(stream::iter(frames), &mut |text| printed.push_str(text))
        .await
        .expect("a closed stream with status updates yields an outcome");

    let task = outcome.task.expect("status updates build the task");
    assert_eq!(task.id.as_str(), TASK_ID);
    assert_eq!(task.context_id.as_str(), CONTEXT_ID);
    assert_eq!(task.status.state, TaskState::Completed);
    assert_eq!(outcome.text, "hello world");
    assert_eq!(printed, "hello world");
}

#[tokio::test]
async fn artifact_updates_accumulate_on_the_task() {
    let artifact = |text: &str, append: bool| {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": "1",
            "result": {
                "artifactUpdate": {
                    "taskId": TASK_ID,
                    "contextId": CONTEXT_ID,
                    "artifact": {
                        "artifactId": "art-1",
                        "parts": [{ "text": text }],
                        "metadata": {
                            "artifact_type": "text",
                            "context_id": CONTEXT_ID,
                            "created_at": "2026-01-01T00:00:00Z",
                            "task_id": TASK_ID
                        }
                    },
                    "append": append,
                    "lastChunk": !append
                }
            }
        })
    };
    let frames = sse(&[
        artifact("one", false),
        artifact("two", true),
        status_frame("TASK_STATE_COMPLETED", None),
    ]);
    let outcome = collect_stream(stream::iter(frames), &mut |_| {})
        .await
        .expect("artifact frames are accepted");

    let task = outcome.task.expect("artifact updates build the task");
    let artifacts = task.artifacts.expect("artifacts recorded");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].parts.len(), 2);
    assert_eq!(task.status.state, TaskState::Completed);
}

#[tokio::test]
async fn a_missing_version_error_frame_is_surfaced() {
    let frames = sse(&[serde_json::json!({
        "jsonrpc": "2.0",
        "id": "1",
        "error": {
            "code": -32009,
            "message": "A2A-Version header is required",
            "data": [{ "reason": "VERSION_NOT_SUPPORTED" }]
        }
    })]);
    let err = collect_stream(stream::iter(frames), &mut |_| {})
        .await
        .expect_err("an error frame fails the stream");
    let rendered = err.to_string();
    assert!(rendered.contains("-32009"), "got {rendered}");
    assert!(rendered.contains("A2A-Version"), "got {rendered}");
}

#[tokio::test]
async fn a_stream_closed_without_any_event_is_an_error() {
    let frames: Vec<Result<Bytes, std::io::Error>> = Vec::new();
    let err = collect_stream(stream::iter(frames), &mut |_| {})
        .await
        .expect_err("an empty stream has no outcome");
    assert!(err.to_string().contains("without a task or message"));
}

#[tokio::test]
async fn an_undecodable_frame_is_an_error() {
    let frames: Vec<Result<Bytes, std::io::Error>> =
        vec![Ok(Bytes::from_static(b"data: not-json\n\n"))];
    let err = collect_stream(stream::iter(frames), &mut |_| {})
        .await
        .expect_err("garbage frames are not skipped silently");
    assert!(err.to_string().contains("undecodable"));
}
