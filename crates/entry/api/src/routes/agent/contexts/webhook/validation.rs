//! Webhook payload validation with size caps.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const MAX_PAYLOAD_SIZE: usize = 1_000_000;
const MAX_TEXT_FIELD_SIZE: usize = 100_000;

#[derive(Debug, thiserror::Error)]
pub enum PayloadValidationError {
    #[error("payload could not be serialized")]
    Serialize(#[source] serde_json::Error),
    #[error("Payload too large: {size} bytes (max: {max})")]
    TooLarge { size: usize, max: usize },
    #[error("serialized payload could not be re-parsed")]
    Reparse(#[source] serde_json::Error),
}

// JSON: webhook payload — arbitrary caller JSON, size-capped before relay.
pub fn validate_json_serializable(value: &serde_json::Value) -> Result<(), PayloadValidationError> {
    let sanitized = sanitize_payload(value, MAX_TEXT_FIELD_SIZE);

    let serialized =
        serde_json::to_string(&sanitized).map_err(PayloadValidationError::Serialize)?;

    if serialized.len() > MAX_PAYLOAD_SIZE {
        return Err(PayloadValidationError::TooLarge {
            size: serialized.len(),
            max: MAX_PAYLOAD_SIZE,
        });
    }

    serde_json::from_str::<serde_json::Value>(&serialized)
        .map_err(PayloadValidationError::Reparse)?;

    Ok(())
}

// JSON: webhook payload — arbitrary caller JSON, size-capped before relay.
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
