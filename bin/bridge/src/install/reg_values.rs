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

#[must_use]
pub fn parse_reg_entries(body: &str) -> Vec<(String, String)> {
    body.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix('"')?;
            let (name, rest) = rest.split_once("\"=\"")?;
            let value = rest.strip_suffix('"')?;
            Some((name.to_owned(), reg_unescape(value)))
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
