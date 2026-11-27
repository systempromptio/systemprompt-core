//! The gateway's model list as the engine's model specs.
//!
//! Both wire protocols are spoken, so no model is dropped here; sizes the
//! instance does not state fall back to conservative defaults, and capabilities
//! are inferred cautiously from the model family and the route.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::projection::{GatewayModel, Protocol as Route};
use systemprompt_engine::llm::{ModelSpec, Protocol, Supports};

const FALLBACK_CONTEXT: u32 = 128_000;

const FALLBACK_OUTPUT: u32 = 8_192;

const CLAUDE_FAMILY: &str = "claude";

pub(crate) fn spec(model: &GatewayModel) -> ModelSpec {
    let protocol = match model.protocol {
        Route::Messages => Protocol::Messages,
        Route::ChatCompletions => Protocol::ChatCompletions,
    };
    let context_window = positive(model.context).unwrap_or(FALLBACK_CONTEXT);
    let max_output = positive(model.output)
        .unwrap_or(FALLBACK_OUTPUT)
        .min(context_window);
    let name = if model.name.trim().is_empty() {
        model.id.clone()
    } else {
        model.name.clone()
    };
    ModelSpec {
        id: model.id.clone(),
        name,
        protocol,
        context_window,
        max_output,
        supports: supports(&model.id, protocol, &model.families),
    }
}

fn positive(value: Option<u64>) -> Option<u32> {
    value
        .filter(|tokens| *tokens > 0)
        .map(|tokens| u32::try_from(tokens).unwrap_or(u32::MAX))
}

fn supports(id: &str, protocol: Protocol, families: &[String]) -> Supports {
    let lowered = id.to_ascii_lowercase();
    let base = lowered.rsplit('/').next().unwrap_or(lowered.as_str());
    let any = |needles: &[&str]| needles.iter().any(|needle| base.contains(needle));
    let claude_family = families.iter().any(|family| family == CLAUDE_FAMILY);
    let claude = base.contains(CLAUDE_FAMILY) || (protocol == Protocol::Messages && claude_family);
    if claude {
        let legacy = base.contains("claude-3") && !base.contains("claude-3-7");
        return Supports {
            images: true,
            reasoning: !legacy,
            tool_calls: true,
            caching: protocol == Protocol::Messages,
        };
    }
    Supports {
        images: any(&[
            "gpt-4o", "gpt-4.1", "gpt-5", "gemini", "vision", "-vl", "pixtral",
        ]),
        reasoning: ["o1", "o3", "o4"]
            .iter()
            .any(|prefix| base.starts_with(prefix))
            || any(&["gpt-5", "reason", "thinking", "-r1"]),
        tool_calls: true,
        caching: false,
    }
}
