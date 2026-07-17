//! Minimal Adaptive Card rendering.
//!
//! Agent replies are plain text or markdown; this renders them into the single
//! Adaptive Card attachment Teams expects, chunking long output across multiple
//! `TextBlock` elements so it is not packed into one unbounded block —
//! mirroring the Slack Block Kit renderer's chunking.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::{Value, json};
use systemprompt_models::text::chunk_text;

/// Soft per-`TextBlock` chunk size, matching the Slack section limit for
/// symmetry across surfaces.
const TEXT_BLOCK_LIMIT: usize = 3000;

const ADAPTIVE_CARD_CONTENT_TYPE: &str = "application/vnd.microsoft.card.adaptive";

/// Render agent text into the `attachments` array of one Adaptive Card.
#[must_use]
pub fn render_card(text: &str) -> Value {
    let body: Vec<Value> = chunk_text(text, TEXT_BLOCK_LIMIT)
        .into_iter()
        .map(|chunk| {
            json!({
                "type": "TextBlock",
                "text": chunk,
                "wrap": true,
            })
        })
        .collect();

    json!([{
        "contentType": ADAPTIVE_CARD_CONTENT_TYPE,
        "content": {
            "type": "AdaptiveCard",
            "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
            "version": "1.5",
            "body": body,
        },
    }])
}
