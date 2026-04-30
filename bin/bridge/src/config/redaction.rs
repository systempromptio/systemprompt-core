use std::fs;

const REDACTED: &str = "***REDACTED***";

const SENSITIVE_KEY_FRAGMENTS: &[&str] = &[
    "secret",
    "credential",
    "auth",
    "pat",
    "token",
    "password",
    "key",
    "pubkey",
    "session",
];

#[must_use]
pub fn redacted_config() -> Option<String> {
    let path = super::config_path()?;
    let raw = fs::read_to_string(&path).ok()?;
    let mut value: toml::Value = toml::from_str(&raw).ok()?;
    redact(&mut value);
    Some(toml::to_string_pretty(&value).unwrap_or_else(|_| String::from("# redaction failed\n")))
}

fn redact(value: &mut toml::Value) {
    match value {
        toml::Value::Table(map) => {
            for (k, v) in map.iter_mut() {
                if is_sensitive_key(k) {
                    *v = toml::Value::String(REDACTED.to_string());
                } else {
                    redact(v);
                }
            }
        },
        toml::Value::Array(arr) => {
            for v in arr.iter_mut() {
                redact(v);
            }
        },
        _ => {},
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SENSITIVE_KEY_FRAGMENTS
        .iter()
        .any(|frag| lower.contains(frag))
}
