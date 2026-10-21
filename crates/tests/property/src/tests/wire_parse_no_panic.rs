use proptest::prelude::*;
use serde_json::{Map, Value};
use systemprompt_wire::anthropic::BetaHeader;
use systemprompt_wire::inspect::{SurfaceBudget, sse_string_leaves, string_leaves};
use systemprompt_wire::sse::{frame_end, upstream_error_message};
use systemprompt_wire::{anthropic, gemini, openai_chat, openai_responses};

const KEYS: &[&str] = &[
    "id",
    "type",
    "role",
    "model",
    "content",
    "text",
    "stop_reason",
    "usage",
    "input_tokens",
    "output_tokens",
    "choices",
    "message",
    "finish_reason",
    "tool_calls",
    "function",
    "arguments",
    "output",
    "status",
    "summary",
    "candidates",
    "parts",
    "finishReason",
    "usageMetadata",
    "promptFeedback",
    "functionCall",
    "error",
    "name",
    "input",
    "thinking",
    "signature",
];

fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        any::<f64>().prop_map(Value::from),
        ".{0,16}".prop_map(Value::String),
        prop::sample::select(KEYS).prop_map(|k| Value::String(k.to_owned())),
    ];
    leaf.prop_recursive(5, 64, 8, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec(
                (prop::sample::select(KEYS).prop_map(str::to_owned), inner),
                0..8
            )
            .prop_map(|kv| Value::Object(kv.into_iter().collect::<Map<_, _>>())),
        ]
    })
}

fn parse_all(value: &Value) {
    let _ = anthropic::parse_response(value, "m");
    let _ = anthropic::buffered_defect(value);
    let _ = openai_chat::parse_response(value, "m");
    let _ = openai_chat::buffered_defect(value);
    let _ = openai_responses::parse_response_object(value, "m");
    let _ = openai_responses::buffered_defect(value);
    let _ = gemini::parse_response(value, "m");
    let _ = gemini::buffered_defect(value);
    let _ = upstream_error_message(value);
}

proptest! {
    #[test]
    fn wire_parse_no_panic_on_json(value in arb_json()) {
        parse_all(&value);
    }

    #[test]
    fn wire_parse_no_panic_on_bytes(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
            parse_all(&value);
        }
        if let Some(end) = frame_end(&bytes) {
            prop_assert!(end <= bytes.len());
        }
        let _ = string_leaves(&bytes, SurfaceBudget::default());
        let _ = sse_string_leaves(&bytes, SurfaceBudget::default());
        let _ = BetaHeader::parse(&String::from_utf8_lossy(&bytes));
    }

    #[test]
    fn wire_parse_no_panic_on_sse_frames(values in prop::collection::vec(arb_json(), 0..6)) {
        let frames: String = values.iter().map(|v| format!("data: {v}\n\n")).collect();
        let _ = sse_string_leaves(frames.as_bytes(), SurfaceBudget::default());
        let _ = string_leaves(frames.as_bytes(), SurfaceBudget::default());
    }
}
