pub fn validate_json_serializable(value: &serde_json::Value) -> Result<(), String> {
    const MAX_PAYLOAD_SIZE: usize = 1_000_000;
    const MAX_TEXT_FIELD_SIZE: usize = 100_000;

    let sanitized = sanitize_payload(value, MAX_TEXT_FIELD_SIZE);

    let serialized = serde_json::to_string(&sanitized)
        .map_err(|e| format!("Failed to serialize to string: {e}"))?;

    if serialized.len() > MAX_PAYLOAD_SIZE {
        return Err(format!(
            "Payload too large: {} bytes (max: {})",
            serialized.len(),
            MAX_PAYLOAD_SIZE
        ));
    }

    serde_json::from_str::<serde_json::Value>(&serialized)
        .map_err(|e| format!("Re-parsing failed: {e}"))?;

    Ok(())
}

pub fn sanitize_payload(value: &serde_json::Value, max_text_size: usize) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            if s.len() > max_text_size {
                serde_json::Value::String(format!(
                    "{}... [truncated from {} bytes]",
                    &s[..max_text_size.min(s.len())],
                    s.len()
                ))
            } else {
                serde_json::Value::String(s.clone())
            }
        },
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| sanitize_payload(v, max_text_size))
                .collect(),
        ),
        serde_json::Value::Object(obj) => {
            let sanitized: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.clone(), sanitize_payload(v, max_text_size)))
                .collect();
            serde_json::Value::Object(sanitized)
        },
        other => other.clone(),
    }
}
