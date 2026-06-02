//! `.reg` serialisation for the Claude Desktop managed-policy profile.
//!
//! [`render_reg`] and [`parse_reg_entries`] are inverses: the GUI stages a
//! `.reg` file for the operator to inspect, then the install path parses it
//! back into the `(name, value)` pairs it writes to the registry in-process.
//! Keeping both halves platform-independent lets the round-trip be unit-tested
//! on every target while the Windows-only registry write stays in `windows.rs`.

use super::shared::{ProfileGenInputs, default_models};

const POLICY_SUBKEY: &str = r"SOFTWARE\Policies\Claude";

#[must_use]
pub fn profile_entries(inputs: &ProfileGenInputs) -> Vec<(&'static str, String)> {
    let models = if inputs.models.is_empty() {
        default_models()
    } else {
        inputs.models.clone()
    };
    let models_json = serde_json::to_string(&models).unwrap_or_else(|_| "[]".into());
    vec![
        ("inferenceProvider", "gateway".to_string()),
        ("inferenceGatewayBaseUrl", inputs.gateway_base_url.clone()),
        ("inferenceGatewayApiKey", inputs.api_key.clone()),
        ("inferenceGatewayAuthScheme", "bearer".to_string()),
        ("inferenceModels", models_json),
    ]
}

#[must_use]
pub fn render_reg(elevated: bool, inputs: &ProfileGenInputs) -> String {
    let hive = if elevated {
        "HKEY_LOCAL_MACHINE"
    } else {
        "HKEY_CURRENT_USER"
    };
    let mut out = String::new();
    out.push_str("Windows Registry Editor Version 5.00\r\n\r\n");
    out.push_str(&format!("[{hive}\\{POLICY_SUBKEY}]\r\n"));
    for (name, value) in profile_entries(inputs) {
        out.push_str(&format!("\"{name}\"=\"{}\"\r\n", reg_escape(&value)));
    }
    out
}

#[must_use]
pub fn parse_reg_entries(body: &str) -> Vec<(String, String)> {
    body.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix('"')?;
            let (name, rest) = rest.split_once("\"=\"")?;
            let value = rest.strip_suffix('"')?;
            Some((name.to_string(), reg_unescape(value)))
        })
        .collect()
}

fn reg_escape(s: &str) -> String {
    s.replace('\\', r"\\").replace('"', "\\\"")
}

fn reg_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(next) => out.push(next),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}
