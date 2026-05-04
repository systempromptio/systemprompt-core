use bytes::Bytes;

pub(super) fn openai_sse_to_anthropic_sse(bytes: &Bytes, model: &str) -> Bytes {
    let text = String::from_utf8_lossy(bytes);
    let mut output = String::new();

    for line in text.lines() {
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        if data.trim() == "[DONE]" {
            push_sse_frame(&mut output, &serde_json::json!({ "type": "message_stop" }));
            continue;
        }
        let Ok(chunk) = serde_json::from_str::<OpenAiStreamChunk>(data) else {
            continue;
        };
        append_anthropic_frames(&mut output, &chunk, model);
    }

    Bytes::from(output)
}

fn append_anthropic_frames(output: &mut String, chunk: &OpenAiStreamChunk, model: &str) {
    for choice in &chunk.choices {
        if choice.delta.role.is_some() {
            let id = chunk.id.as_deref().unwrap_or("msg_openai");
            push_sse_frame(
                output,
                &serde_json::json!({
                    "type": "message_start",
                    "message": {
                        "id": id,
                        "type": "message",
                        "role": "assistant",
                        "model": model,
                        "usage": { "input_tokens": 0, "output_tokens": 0 },
                    },
                }),
            );
            push_sse_frame(
                output,
                &serde_json::json!({
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": { "type": "text", "text": "" },
                }),
            );
        }
        if let Some(text) = choice.delta.content.as_deref() {
            if !text.is_empty() {
                push_sse_frame(
                    output,
                    &serde_json::json!({
                        "type": "content_block_delta",
                        "index": 0,
                        "delta": { "type": "text_delta", "text": text },
                    }),
                );
            }
        }
        if let Some(finish) = choice.finish_reason.as_deref() {
            if !finish.is_empty() && finish != "null" {
                let stop_reason = if finish == "stop" { "end_turn" } else { finish };
                push_sse_frame(
                    output,
                    &serde_json::json!({
                        "type": "message_delta",
                        "delta": { "stop_reason": stop_reason },
                        "usage": { "output_tokens": 0 },
                    }),
                );
            }
        }
    }
}

fn push_sse_frame(output: &mut String, value: &serde_json::Value) {
    output.push_str("data: ");
    if let Ok(encoded) = serde_json::to_string(value) {
        output.push_str(&encoded);
    }
    output.push_str("\n\n");
}

#[derive(serde::Deserialize)]
struct OpenAiStreamChunk {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(serde::Deserialize)]
struct OpenAiStreamChoice {
    #[serde(default)]
    delta: OpenAiStreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(serde::Deserialize, Default)]
struct OpenAiStreamDelta {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Option<String>,
}
