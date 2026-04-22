use bytes::Bytes;
use serde_json::Value;

pub fn flatten_system_prompt(system: &Value) -> Option<String> {
    match system {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Array(arr) => {
            let joined = arr
                .iter()
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n");
            if joined.is_empty() {
                None
            } else {
                Some(joined)
            }
        },
        _ => None,
    }
}

pub fn flatten_message_content(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => {
            let mut out = String::new();
            for block in blocks {
                append_block(&mut out, block);
            }
            out
        },
        _ => serialize_or_warn(content, "flatten: content serialize failed"),
    }
}

fn append_block(out: &mut String, block: &Value) {
    let kind = block.get("type").and_then(Value::as_str).unwrap_or("");
    if kind == "text" {
        if let Some(text) = block.get("text").and_then(Value::as_str) {
            push_with_sep(out, text);
        }
        return;
    }
    match serde_json::to_string(block) {
        Ok(s) => push_with_sep(out, &s),
        Err(e) => tracing::warn!(error = %e, "flatten: block serialize failed"),
    }
}

fn push_with_sep(out: &mut String, fragment: &str) {
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(fragment);
}

fn serialize_or_warn(value: &Value, context: &'static str) -> String {
    match serde_json::to_string(value) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "{context}");
            String::new()
        },
    }
}

pub fn rewrite_request_model(raw_body: Bytes, upstream_model: &str) -> anyhow::Result<Bytes> {
    serde_json::from_slice::<Value>(&raw_body).map_or_else(
        |_| Ok(raw_body),
        |mut v| {
            if let Some(obj) = v.as_object_mut() {
                obj.insert(
                    "model".to_string(),
                    Value::String(upstream_model.to_string()),
                );
            }
            serde_json::to_vec(&v)
                .map(Bytes::from)
                .map_err(|e| anyhow::anyhow!("re-serialize request body with upstream model: {e}"))
        },
    )
}

pub fn parse_served_model(response_bytes: &Bytes) -> Option<String> {
    serde_json::from_slice::<Value>(response_bytes)
        .ok()
        .and_then(|v| {
            v.get("model")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        })
}
