//! Rendering a [`PolicyDocument`] as the XML plist macOS Managed Preferences
//! expect. Kept platform-independent so the bytes are testable everywhere.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use super::document::{PolicyDocument, PolicyDocumentValue};

const HEAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
"#;
const TAIL: &str = "</dict>\n</plist>\n";

#[must_use]
pub fn render_plist(doc: &PolicyDocument) -> String {
    let mut out = String::from(HEAD);
    write_entries(&mut out, doc, "  ");
    out.push_str(TAIL);
    out
}

fn write_entries(out: &mut String, entries: &BTreeMap<String, PolicyDocumentValue>, indent: &str) {
    for (key, value) in entries {
        out.push_str(&format!("{indent}<key>{}</key>\n", escape(key)));

        write_value(out, value, indent);
    }
}

fn write_value(out: &mut String, value: &PolicyDocumentValue, indent: &str) {
    match value {
        PolicyDocumentValue::Str(s) => {
            out.push_str(&format!("{indent}<string>{}</string>\n", escape(s)));
        },
        PolicyDocumentValue::Bool(true) => {
            out.push_str(&format!("{indent}<true/>\n"));
        },
        PolicyDocumentValue::Bool(false) => {
            out.push_str(&format!("{indent}<false/>\n"));
        },
        PolicyDocumentValue::StrList(items) => {
            out.push_str(&format!("{indent}<array>\n"));

            for item in items {
                out.push_str(&format!("{indent}  <string>{}</string>\n", escape(item)));
            }
            out.push_str(&format!("{indent}</array>\n"));
        },
        PolicyDocumentValue::Dicts(dicts) => {
            out.push_str(&format!("{indent}<array>\n"));

            for dict in dicts {
                out.push_str(&format!("{indent}  <dict>\n"));

                write_entries(out, dict, &format!("{indent}    "));
                out.push_str(&format!("{indent}  </dict>\n"));
            }
            out.push_str(&format!("{indent}</array>\n"));
        },
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
