use std::time::Duration;

use futures_util::StreamExt;
use harness_llm::{Event, FinishReason, LlmError, Protocol, Request, Usage, stream};
use tokio_util::sync::CancellationToken;

use crate::support::{BASE, Canned, endpoint, model, open, run};

#[tokio::test]
async fn chat_stream_end_to_end() {
    let transport = Canned::new(
        200,
        &[
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel",
            "lo\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":1}}\n\ndata: [DONE]\n\n",
        ],
    );
    let events: Vec<Event> = run(&transport, Protocol::ChatCompletions)
        .await
        .into_iter()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        events,
        vec![
            Event::TextDelta("Hello".into()),
            Event::Usage(Usage {
                input: 5,
                output: 1,
                ..Usage::default()
            }),
            Event::Finish {
                reason: FinishReason::Stop
            },
        ]
    );
}

#[tokio::test]
async fn non_success_status_is_typed() {
    let mut transport = Canned::new(429, &["{\"error\":{\"message\":\"slow down\"}}"]);
    transport.headers = vec![("retry-after".into(), "4".into())];
    let result = open(&transport, Protocol::Messages).await;
    assert!(matches!(
        result,
        Err(LlmError::Status { status: 429, retry_after: Some(wait), ref body_excerpt })
            if wait == Duration::from_secs(4) && body_excerpt == "slow down"
    ));
}

#[tokio::test]
async fn overflow_status_is_detected() {
    let transport = Canned::new(
        400,
        &[
            "{\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"prompt is too long: 300000 tokens\"}}",
        ],
    );
    let result = open(&transport, Protocol::Messages).await;
    assert!(matches!(
        result,
        Err(LlmError::ContextOverflow {
            status: Some(400),
            ..
        })
    ));
}

#[tokio::test]
async fn cancellation_aborts_an_open_stream() {
    let mut transport = Canned::new(
        200,
        &["data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"a\"}}]}\n\n"],
    );
    transport.hang = true;
    let cancel = CancellationToken::new();
    let request = Request::new(model(Protocol::ChatCompletions));
    let mut events = stream(&transport, &endpoint(BASE), &request, cancel.clone())
        .await
        .unwrap()
        .events;
    assert_eq!(
        events.next().await.and_then(Result::ok),
        Some(Event::TextDelta("a".into()))
    );
    cancel.cancel();
    assert!(matches!(events.next().await, Some(Err(LlmError::Aborted))));
    assert!(events.next().await.is_none());
}

#[tokio::test]
async fn cancelled_before_send_is_aborted() {
    let transport = Canned::new(200, &[]);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let request = Request::new(model(Protocol::ChatCompletions));
    let result = stream(&transport, &endpoint(BASE), &request, cancel).await;
    assert!(matches!(result, Err(LlmError::Aborted)));
}

#[tokio::test]
async fn early_eof_is_truncated() {
    let transport = Canned::new(
        200,
        &["data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"a\"}}]}\n\n"],
    );
    let events = run(&transport, Protocol::ChatCompletions).await;
    assert!(matches!(events.first(), Some(Ok(Event::TextDelta(_)))));
    assert!(matches!(events.last(), Some(Err(LlmError::Truncated))));
}

#[tokio::test]
async fn an_opened_response_hands_back_its_head() {
    let mut transport = Canned::new(201, &["data: [DONE]\n\n"]);
    transport.headers = vec![
        ("X-Memory-Ids".to_owned(), "fact-1,fact-2".to_owned()),
        ("content-type".to_owned(), "text/event-stream".to_owned()),
    ];
    let request = Request::new(model(Protocol::ChatCompletions));
    let response = stream(
        &transport,
        &endpoint(BASE),
        &request,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(response.head.status, 201);
    assert_eq!(response.head.header("x-memory-ids"), Some("fact-1,fact-2"));
    assert_eq!(response.head.header("retry-after"), None);
    let shown = format!("{:?}", response.head);
    assert!(shown.contains("X-Memory-Ids"), "{shown}");
    assert!(!shown.contains("fact-1"), "{shown}");
}
