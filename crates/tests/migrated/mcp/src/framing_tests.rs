//! Newline-delimited framing from a local server: line endings, blank lines,
//! several messages per chunk, split characters and large frames.

use std::time::Duration;

use serde_json::{Value, json};
use systemprompt_engine::mcp::{McpClient, RequestOptions};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const MIB: usize = 1024 * 1024;

/// Starts a client whose peer answers the first request with `answer`
/// (built from the request id), written in `chunk`-byte pieces.
fn raw_peer(answer: impl FnOnce(&Value) -> Vec<u8> + Send + 'static, chunk: usize) -> McpClient {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        let (read, mut write) = tokio::io::split(server_io);
        let mut lines = BufReader::new(read).lines();
        let Ok(Some(line)) = lines.next_line().await else {
            return;
        };
        let request: Value = serde_json::from_str(&line).unwrap();
        let bytes = answer(&request);
        for piece in bytes.chunks(chunk) {
            if write.write_all(piece).await.is_err() {
                return;
            }
            tokio::task::yield_now().await;
        }
        while let Ok(Some(_)) = lines.next_line().await {}
    });
    let (read, write) = tokio::io::split(client_io);
    McpClient::from_stream(read, write)
}

fn result_frame(request: &Value, payload: &str) -> Vec<u8> {
    serde_json::to_vec(
        &json!({"jsonrpc": "2.0", "id": request["id"].clone(), "result": {"text": payload}}),
    )
    .unwrap()
}

async fn ask(client: &McpClient) -> Result<Value, systemprompt_engine::mcp::McpError> {
    client
        .request(
            "ping",
            None,
            RequestOptions::with_timeout(Duration::from_secs(20)),
        )
        .await
}

#[tokio::test]
async fn crlf_blank_lines_several_messages_per_chunk_and_split_characters_are_framed() {
    let client = raw_peer(
        |request| {
            let mut bytes = b"\r\n\n   \n".to_vec();
            bytes.extend_from_slice(
                br#"{"jsonrpc":"2.0","method":"notifications/message","params":{}}"#,
            );
            bytes.extend_from_slice(b"\r\n");
            bytes.extend(result_frame(request, "caf\u{e9} \u{20ac} \u{1f600}"));
            bytes.extend_from_slice(b"\r\n\r\n");
            bytes
        },
        3,
    );
    let answer = ask(&client).await.unwrap();
    assert_eq!(answer["text"], "caf\u{e9} \u{20ac} \u{1f600}");
    client.shutdown().await;
}

#[tokio::test]
async fn a_frame_of_twelve_mebibytes_split_into_pipe_sized_chunks_is_received() {
    let payload = "x".repeat(12 * MIB);
    let expected = payload.len();
    let client = raw_peer(
        move |request| {
            let mut bytes = result_frame(request, &payload);
            bytes.push(b'\n');
            bytes
        },
        64 * 1024,
    );
    let answer = ask(&client).await.unwrap();
    assert_eq!(answer["text"].as_str().map(str::len), Some(expected));
    client.shutdown().await;
}

#[tokio::test]
async fn a_frame_of_exactly_sixteen_mebibytes_is_accepted() {
    let client = raw_peer(
        |request| {
            let envelope = result_frame(request, "").len();
            let mut bytes = result_frame(request, &"y".repeat(16 * MIB - envelope));
            assert_eq!(bytes.len(), 16 * MIB);
            bytes.push(b'\n');
            bytes
        },
        64 * 1024,
    );
    assert!(ask(&client).await.is_ok());
    client.shutdown().await;
}

#[tokio::test]
#[ignore = "parity ledger: there is no frame size limit; a line one byte over sixteen mebibytes is accepted and the connection stays open"]
async fn a_line_one_byte_over_sixteen_mebibytes_closes_the_connection() {
    let client = raw_peer(
        |request| {
            let envelope = result_frame(request, "").len();
            let mut bytes = result_frame(request, &"z".repeat(16 * MIB - envelope + 1));
            bytes.push(b'\n');
            bytes
        },
        64 * 1024,
    );
    assert!(ask(&client).await.is_err());
    assert!(client.is_closed());
}
