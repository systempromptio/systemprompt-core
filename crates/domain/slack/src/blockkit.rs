//! Minimal Block Kit rendering.
//!
//! Agent replies are plain text or markdown; this renders them into the Block
//! Kit `blocks` array Slack expects, chunking past Slack's 3000-character
//! per-section limit so long agent output is not silently truncated.

use serde_json::{Value, json};
use systemprompt_models::text::chunk_text;

/// Slack's hard limit on a single section block's text length.
const SECTION_TEXT_LIMIT: usize = 3000;

/// Render agent text into a Block Kit `blocks` array of `section`/`mrkdwn`
/// blocks.
#[must_use]
pub fn render_blocks(text: &str) -> Value {
    let mut blocks = Vec::new();
    for chunk in chunk_text(text, SECTION_TEXT_LIMIT) {
        blocks.push(json!({
            "type": "section",
            "text": { "type": "mrkdwn", "text": chunk },
        }));
    }
    Value::Array(blocks)
}
