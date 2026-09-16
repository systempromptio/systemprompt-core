//! The Windows registry profile as a document: rendering policy values to a
//! `.reg` body and reading them back. Format only — what the values *are*
//! belongs to the host that writes them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) const POLICY_SUBKEY: &str = r"SOFTWARE\Policies\Claude";

#[must_use]
pub fn render_reg_values(elevated: bool, entries: &[(&str, String)]) -> String {
    let hive = if elevated {
        "HKEY_LOCAL_MACHINE"
    } else {
        "HKEY_CURRENT_USER"
    };
    let mut out = String::new();
    out.push_str("Windows Registry Editor Version 5.00\r\n\r\n");
    out.push_str(&format!("[{hive}\\{POLICY_SUBKEY}]\r\n"));
    for (name, value) in entries {
        out.push_str(&format!("\"{name}\"=\"{}\"\r\n", reg_escape(value)));
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("staged registry profile line {line} is not a \"name\"=\"value\" entry: {text:?}")]
pub struct RegLineError {
    pub line: usize,
    pub text: String,
}

// Why: a line the parser cannot read is a policy value that would silently
// go unwritten; the whole profile is refused rather than applied partially.
pub fn parse_reg_entries(body: &str) -> Result<Vec<(String, String)>, RegLineError> {
    let mut entries = Vec::new();
    for (index, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("Windows Registry Editor")
            || trimmed.starts_with('[')
            || trimmed.starts_with(';')
        {
            continue;
        }
        let parsed = trimmed
            .strip_prefix('"')
            .and_then(|rest| rest.split_once("\"=\""))
            .and_then(|(name, rest)| rest.strip_suffix('"').map(|value| (name, value)));
        let Some((name, value)) = parsed else {
            return Err(RegLineError {
                line: index + 1,
                text: trimmed.to_owned(),
            });
        };
        entries.push((name.to_owned(), reg_unescape(value)));
    }
    Ok(entries)
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
