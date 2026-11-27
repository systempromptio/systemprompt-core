//! The reader, writer and stderr loops that move bytes between the server
//! process and the client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::ChildStdin;
use tokio::sync::mpsc;
use tracing::{trace, warn};

use super::connection::Shared;
use crate::lsp::framing::{FrameDecoder, FrameError};

const READ_CHUNK: usize = 16 * 1024;
const STDERR_TAIL_LINES: usize = 20;

pub(super) async fn read_loop<R: AsyncRead + Unpin>(mut reader: R, shared: Arc<Shared>) {
    let mut decoder = FrameDecoder::default();
    let mut chunk = vec![0u8; READ_CHUNK];
    let reason = loop {
        let read = tokio::select! {
            () = shared.cancel.cancelled() => return,
            read = reader.read(&mut chunk) => read,
        };
        match read {
            Ok(0) => break "the server closed its output".to_owned(),
            Ok(count) => {
                decoder.push(&chunk[..count]);
                if let Err(error) = dispatch_frames(&mut decoder, &shared) {
                    break format!("the server sent a malformed frame ({error})");
                }
            },
            Err(error) => break format!("reading from the server failed ({error})"),
        }
    };
    shared.mark_dead(&reason);
}

fn dispatch_frames(decoder: &mut FrameDecoder, shared: &Shared) -> Result<(), FrameError> {
    while let Some(body) = decoder.next_frame()? {
        match serde_json::from_slice::<Value>(&body) {
            Ok(message) => shared.on_message(&message),
            Err(error) => {
                warn!(server = %shared.server, %error, "ignored a message that is not JSON");
            },
        }
    }
    Ok(())
}

pub(super) async fn write_loop(
    mut stdin: ChildStdin,
    mut frames: mpsc::UnboundedReceiver<Vec<u8>>,
    shared: Arc<Shared>,
) {
    loop {
        let frame = tokio::select! {
            () = shared.cancel.cancelled() => return,
            frame = frames.recv() => frame,
        };
        let Some(frame) = frame else {
            return;
        };
        let written = async {
            stdin.write_all(&frame).await?;
            stdin.flush().await
        };
        if let Err(error) = written.await {
            shared.mark_dead(&format!("writing to the server failed ({error})"));
            return;
        }
    }
}

pub(super) async fn stderr_loop<R: AsyncRead + Unpin>(mut stderr: R, shared: Arc<Shared>) {
    let mut chunk = vec![0u8; READ_CHUNK];
    let mut partial: Vec<u8> = Vec::new();
    loop {
        let read = tokio::select! {
            () = shared.cancel.cancelled() => return,
            read = stderr.read(&mut chunk) => read,
        };
        let count = match read {
            Ok(0) | Err(_) => return,
            Ok(count) => count,
        };
        partial.extend_from_slice(&chunk[..count]);
        while let Some(end) = partial.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = partial.drain(..=end).collect();
            let text = String::from_utf8_lossy(&line).trim_end().to_owned();
            trace!(server = %shared.server, line = %text, "server stderr");
            push_tail(&shared, text);
        }
        if partial.len() > READ_CHUNK {
            partial.clear();
        }
    }
}

fn push_tail(shared: &Shared, line: String) {
    let mut tail = shared.stderr_tail.lock();
    if tail.len() == STDERR_TAIL_LINES {
        tail.pop_front();
    }
    tail.push_back(line);
}
