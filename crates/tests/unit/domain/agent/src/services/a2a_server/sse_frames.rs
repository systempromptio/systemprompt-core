use std::convert::Infallible;

use axum::response::IntoResponse;
use axum::response::sse::{Event, Sse};
use systemprompt_agent::models::a2a::jsonrpc::RequestId;
use systemprompt_agent::services::a2a_server::errors::JsonRpcErrorBuilder;
use systemprompt_agent::services::a2a_server::streaming::{
    jsonrpc_error_event, stream_response_event, stream_response_frame,
};
use systemprompt_agent_entry::a2a_server::sse_event;
use systemprompt_models::a2a::{StreamResponse, Task};

async fn wire(events: Vec<Event>) -> String {
    let response = Sse::new(futures::stream::iter(
        events.into_iter().map(Ok::<_, Infallible>),
    ))
    .into_response();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("an SSE body collects");
    String::from_utf8(bytes.to_vec()).expect("SSE bytes are UTF-8")
}

fn error() -> JsonRpcErrorBuilder {
    JsonRpcErrorBuilder::internal_error().with_metadata("detail", "Agent not found")
}

#[tokio::test]
async fn stream_frames_serve_the_bytes_a_data_event_of_the_frame_json_serves() {
    let id = RequestId::Number(7);
    let response = StreamResponse::Task(Task::default());
    let result_json =
        serde_json::to_string(&stream_response_frame(response.clone(), &id)).expect("frame JSON");
    let error_json = serde_json::to_string(&error().build(&id)).expect("error JSON");

    let served = wire(vec![
        sse_event(stream_response_event(response, &id)),
        sse_event(jsonrpc_error_event(error(), &id)),
    ])
    .await;
    let previous = wire(vec![
        Event::default().data(result_json.clone()),
        Event::default().data(error_json.clone()),
    ])
    .await;

    assert_eq!(served, previous);
    assert_eq!(
        served,
        format!("data: {result_json}\n\ndata: {error_json}\n\n")
    );
}

#[test]
fn a_stream_frame_carries_the_serialised_jsonrpc_response() {
    let id = RequestId::Number(3);
    let frame = jsonrpc_error_event(error(), &id);
    let expected = serde_json::to_string(&error().build(&id)).expect("error JSON");
    assert_eq!(frame.as_data(), expected);
    assert_eq!(frame.into_data(), expected);
}
