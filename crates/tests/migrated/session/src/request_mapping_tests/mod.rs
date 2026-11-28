//! What the model is sent for a session's history and a prompt's attachments:
//! how stored messages, tool results and compaction summaries map to request
//! turns, how files and data URLs are admitted, stored and sent as media, and
//! how media types are told from content.

mod admission;
mod history;
mod media_types;
mod prompt_content;
mod seed;
mod vision;

use serde_json::{Value, json};
use systemprompt_engine::run::{AttachmentInput, Delivery};
use systemprompt_engine::store::SessionId;

use crate::support::{Scripted, settle};

/// A one-pixel PNG, base64 with no padding.
const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNQSFgAAAHEASFiX4r9AAAAAElFTkSuQmCC";
/// The opening box of a HEIC file, base64.
const HEIC: &str = "AAAAGGZ0eXBoZWljAAAAAG1pZjFoZWlj";
/// Four arbitrary bytes, base64.
const PIXELS: &str = "AAECAw==";
/// A PDF header line, base64.
const PDF: &str = "JVBERi0xLjQK";

fn data_url(mime: &str, data: &str, name: Option<&str>) -> AttachmentInput {
    AttachmentInput::DataUrl {
        url: format!("data:{mime};base64,{data}"),
        name: name.map(str::to_owned),
    }
}

// JSON: a Messages text block as the instance recorded it.
fn text_block(text: &str) -> Value {
    json!({"type": "text", "text": text})
}

// JSON: a Messages image block as the instance recorded it.
fn image_block(mime: &str, data: &str) -> Value {
    json!({"type": "image", "source": {"type": "base64", "media_type": mime, "data": data}})
}

// JSON: a Messages user turn of text blocks as the instance recorded it.
fn user_turn(texts: &[&str]) -> Value {
    let content: Vec<Value> = texts.iter().map(|text| text_block(text)).collect();
    json!({"role": "user", "content": content})
}

// JSON: the block types of one recorded Messages turn.
fn block_types(turn: &Value) -> Vec<&str> {
    turn["content"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|block| block["type"].as_str())
                .collect()
        })
        .unwrap_or_default()
}

/// Sends a prompt, waits for the run to end and returns the conversation turns
/// of the last model request.
// JSON: the `messages` array of the request body the instance recorded.
async fn ask(scripted: &Scripted, id: &SessionId, text: &str) -> Value {
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(id, text, Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, id).await;
    scripted
        .model_bodies()
        .last()
        .expect("a model request was sent")["messages"]
        .clone()
}
