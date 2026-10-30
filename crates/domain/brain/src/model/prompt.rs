//! The extraction prompt: the kind's versioned extractor prompt as the
//! system turn, and the subject hints and source text as the user turn.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::brain::{BrainPrompt, KindDecl, SourceText};

#[must_use]
pub fn build_prompt(
    kind: &KindDecl,
    system: &str,
    text: &SourceText,
    hints: &[String],
) -> BrainPrompt {
    let mut lines = vec![
        format!("Fact kind: {}", kind.kind),
        format!("Subject kind: {}", kind.subject),
    ];
    if let Some(description) = &kind.description {
        lines.push(format!("Kind description: {description}"));
    }
    if !hints.is_empty() {
        lines.push(format!("Subject hints: {}", hints.join(", ")));
    }
    lines.push(String::new());
    lines.push("Source nodes:".to_owned());
    if text.nodes.is_empty() {
        lines.push(text.text.clone());
    } else {
        lines.extend(
            text.nodes
                .iter()
                .map(|node| format!("[{} {}] {}", node.node_id, node.hash8, node.text)),
        );
    }
    BrainPrompt {
        system: system.to_owned(),
        user: lines.join("\n"),
    }
}
