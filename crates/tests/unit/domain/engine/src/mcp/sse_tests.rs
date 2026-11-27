use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use systemprompt_engine::mcp::{CancellationToken, RemoteHeaders, ServerStatus};
use tokio::sync::mpsc;

use crate::mcp::support::http::{HttpServer, Recorded, Response, mcp_reply, rpc_frame, tools};
use crate::mcp::support::manager::{manager, remote, settled, status_of};
use crate::mcp::support::stream::initialize_result;

type Stream = Arc<Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>>;

const OPENING: [&str; 2] = [
    "\u{feff}event: endpoint\r\ndata: /messages?session=1\r",
    "\n\r\n: keep-alive\n\nid: 4\nretry: 100\nevent: ping\n\n",
];

fn push(stream: &Stream, text: &str) {
    let guard = stream.lock().unwrap();
    guard
        .as_ref()
        .unwrap()
        .send(text.as_bytes().to_vec())
        .unwrap();
}

fn open_stream(stream: &Stream) -> Response {
    let (tx, rx) = mpsc::unbounded_channel();
    for chunk in OPENING {
        tx.send(chunk.as_bytes().to_vec()).unwrap();
    }
    *stream.lock().unwrap() = Some(tx);
    Response::stream(rx)
}

fn legacy(request: &Recorded, stream: &Stream, listed: &Value) -> Response {
    if request.method == "GET" {
        return open_stream(stream);
    }
    if request.path == "/" {
        return Response::status(405);
    }
    match request.rpc_method() {
        Some("initialize") => {
            let body = rpc_frame(request, &initialize_result("2024-11-05"));
            push(stream, &format!("event: message\rdata: {body}\r\r\n"));
        },
        Some("tools/list") => {
            let body = rpc_frame(request, &json!({"tools": listed})).to_string();
            let (head, tail) = body.split_at(body.find("\"result\"").unwrap());
            push(
                stream,
                &format!("event: other\ndata: {{}}\n\ndata: {head}\ndata: {tail}\n\n"),
            );
        },
        Some("tools/call") => {
            let body = rpc_frame(
                request,
                &json!({"content": [{"type": "text", "text": "legacy"}]}),
            );
            push(stream, &format!("data: {body}\n\n"));
        },
        _ => {},
    }
    Response::status(202)
}

#[tokio::test]
async fn a_refused_post_falls_back_to_legacy_sse() {
    let stream: Stream = Arc::new(Mutex::new(None));
    let listed = tools(&["echo", "add"]);
    let handler_stream = Arc::clone(&stream);
    let server = HttpServer::start(move |request| legacy(request, &handler_stream, &listed)).await;
    let manager = manager();
    let config = remote(server.url("/"), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([("old".to_owned(), config)]));
    settled(&manager).await;
    assert_eq!(
        status_of(&manager, "old"),
        ServerStatus::Connected { tools: 2 }
    );
    let info = manager
        .client("old")
        .unwrap()
        .server_info()
        .cloned()
        .unwrap();
    assert_eq!(info.protocol_version, "2024-11-05");
    let result = manager
        .call("old_echo", Value::Null, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.text, "legacy");
    let posted = server
        .requests()
        .into_iter()
        .filter(|request| request.path == "/messages?session=1");
    assert_eq!(posted.count(), 4);
    manager.shutdown().await;
}

#[tokio::test]
async fn an_endpoint_on_another_origin_is_refused() {
    let server = HttpServer::start(|request| {
        if request.method == "GET" {
            return Response::event_stream(&[
                "event: endpoint\ndata: http://elsewhere.invalid/m\n\n",
            ]);
        }
        Response::status(404)
    })
    .await;
    let manager = manager();
    let config = remote(server.url("/"), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([("far".to_owned(), config)]));
    settled(&manager).await;
    let ServerStatus::Failed { error } = status_of(&manager, "far") else {
        panic!("expected a failed server");
    };
    assert!(error.contains("another origin"));
    assert!(server.requests().iter().all(|request| request.path == "/"));
}

#[tokio::test]
async fn a_post_can_answer_with_an_event_stream() {
    let listed = tools(&["echo"]);
    let server = HttpServer::start(move |request| {
        if request.rpc_method() == Some("tools/list") {
            let body = rpc_frame(request, &json!({"tools": listed}));
            let event = format!(": comment\n\nevent: message\ndata: {body}\n\n");
            let halves: [&str; 2] = event.split_at(event.len() / 2).into();
            return Response::event_stream(&halves);
        }
        mcp_reply(request, &listed)
    })
    .await;
    let manager = manager();
    let config = remote(server.url("/mcp"), RemoteHeaders::default());
    manager.reconcile(BTreeMap::from([("ev".to_owned(), config)]));
    settled(&manager).await;
    assert_eq!(
        status_of(&manager, "ev"),
        ServerStatus::Connected { tools: 1 }
    );
    assert_eq!(manager.tools()[0].qualified_name, "ev_echo");
    manager.shutdown().await;
}
