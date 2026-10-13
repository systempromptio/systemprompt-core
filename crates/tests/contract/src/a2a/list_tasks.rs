use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::models::a2a::protocol::{
    A2aJsonRpcRequest, A2aRequestParams, A2aResponse, ListTasksParams, ListTasksResult,
};
use systemprompt_agent::services::a2a_server::handlers::request::list_tasks::{
    DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE, PageToken, clamp_page_size, trim_history,
};
use systemprompt_identifiers::{ContextId, MessageId};
use systemprompt_models::a2a::{Message, MessageRole, Part, Task, TaskState};

use super::schema::assert_valid;

const CTX: &str = "00000000-0000-4000-8000-000000000001";

fn message(n: usize) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::text(format!("m{n}"))],
        message_id: MessageId::new(format!("m-{n}")),
        task_id: None,
        context_id: ContextId::generate(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn task_with_history(len: usize) -> Task {
    Task {
        history: Some((0..len).map(message).collect()),
        ..Task::default()
    }
}

#[test]
fn page_size_defaults_to_50_and_clamps_to_1_100() {
    assert_eq!(DEFAULT_PAGE_SIZE, 50);
    assert_eq!(MAX_PAGE_SIZE, 100);
    assert_eq!(clamp_page_size(None), 50);
    assert_eq!(clamp_page_size(Some(0)), 1);
    assert_eq!(clamp_page_size(Some(7)), 7);
    assert_eq!(clamp_page_size(Some(1000)), 100);
}

#[test]
fn page_token_round_trips_and_is_opaque() {
    for offset in [0, 1, 50, 4_000_000] {
        let token = PageToken::from_offset(offset).encode();
        assert_eq!(PageToken::decode(&token).unwrap().offset(), offset);
    }
}

#[test]
fn malformed_page_tokens_are_rejected() {
    for token in ["!!!", "bm90LWEtbnVtYmVy", "LTE"] {
        assert!(
            PageToken::decode(token).is_err(),
            "{token} must be rejected"
        );
    }
}

#[test]
fn history_length_keeps_the_most_recent_messages() {
    let mut task = task_with_history(5);
    trim_history(&mut task, Some(2));
    let kept: Vec<_> = task
        .history
        .unwrap()
        .into_iter()
        .map(|m| m.message_id.to_string())
        .collect();
    assert_eq!(kept, ["m-3", "m-4"]);
}

#[test]
fn history_length_zero_drops_history_and_none_keeps_it() {
    let mut task = task_with_history(3);
    trim_history(&mut task, Some(0));
    assert!(task.history.is_none());

    let mut task = task_with_history(3);
    trim_history(&mut task, None);
    assert_eq!(task.history.unwrap().len(), 3);
}

#[test]
fn list_tasks_request_parses_every_filter() {
    let request = A2aJsonRpcRequest {
        jsonrpc: "2.0".into(),
        method: "ListTasks".into(),
        params: serde_json::json!({
            "contextId": CTX,
            "status": "TASK_STATE_WORKING",
            "pageSize": 10,
            "pageToken": PageToken::from_offset(10).encode(),
            "historyLength": 2,
            "statusTimestampAfter": "2026-10-01T00:00:00Z",
            "includeArtifacts": true
        }),
        id: RequestId::Number(1),
    };
    let Ok(A2aRequestParams::ListTasks(params)) = request.parse_request() else {
        panic!("ListTasks must route");
    };
    assert_eq!(params.context_id, Some(ContextId::try_new(CTX).unwrap()));
    assert_eq!(params.status, Some(TaskState::Working));
    assert_eq!(params.page_size, Some(10));
    assert_eq!(params.history_length, Some(2));
    assert_eq!(params.include_artifacts, Some(true));
    assert!(params.status_timestamp_after.is_some());
    assert_valid("ListTasksRequest", &request.params);
}

#[test]
fn empty_list_tasks_params_are_valid() {
    let params: ListTasksParams = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(params, ListTasksParams::default());
}

#[test]
fn list_tasks_result_validates_against_the_schema() {
    let result = ListTasksResult {
        tasks: vec![task_with_history(1)],
        next_page_token: PageToken::from_offset(1).encode(),
        page_size: 1,
        total_size: None,
    };
    let response = A2aResponse::list_tasks(result, RequestId::Number(3));
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["id"], 3);
    assert_valid("ListTasksResponse", &json["result"]);
}

#[test]
fn last_page_has_an_empty_next_token() {
    let result = ListTasksResult {
        tasks: vec![],
        next_page_token: String::new(),
        page_size: 50,
        total_size: None,
    };
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["nextPageToken"], "");
    assert_valid("ListTasksResponse", &json);
}
