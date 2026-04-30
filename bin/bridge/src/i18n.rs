// Tiny Fluent-subset loader for handler-emitted strings.
// Mirrors web/js/i18n.js: parses `id = value` lines and `{ $arg }` placeables.
// Locale negotiation reads the LANG env var; en-US is always available as
// the embedded fallback. Drop a `web/i18n/<locale>/bridge.ftl` file and run
// the bridge with `LANG=<locale>.UTF-8` to flip handler messages.

use std::collections::HashMap;
use std::sync::OnceLock;

const FALLBACK_FTL: &str = include_str!("../web/i18n/en-US/bridge.ftl");

static CATALOG: OnceLock<Catalog> = OnceLock::new();

struct Catalog {
    messages: HashMap<String, String>,
}

fn parse(src: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some(eq) = trimmed.find('=') else {
            continue;
        };
        let id = trimmed[..eq].trim();
        let value = trimmed[eq + 1..].trim();
        if !id.is_empty() {
            out.insert(id.to_string(), value.to_string());
        }
    }
    out
}

fn negotiated_locale() -> Option<String> {
    let raw = std::env::var("LANG").ok()?;
    let head = raw.split('.').next()?;
    Some(head.replace('_', "-"))
}

fn load_external(locale: &str) -> Option<HashMap<String, String>> {
    let cfg_dir = dirs::config_dir()?.join("systemprompt").join("i18n");
    let candidate = cfg_dir.join(locale).join("bridge.ftl");
    let raw = std::fs::read_to_string(candidate).ok()?;
    Some(parse(&raw))
}

fn catalog() -> &'static Catalog {
    CATALOG.get_or_init(|| {
        let mut messages = parse(FALLBACK_FTL);
        if let Some(locale) = negotiated_locale()
            && locale != "en-US"
                && let Some(extra) = load_external(&locale) {
                    for (k, v) in extra {
                        messages.insert(k, v);
                    }
                }
        Catalog { messages }
    })
}

pub fn t(id: &str) -> String {
    catalog()
        .messages
        .get(id)
        .cloned()
        .unwrap_or_else(|| id.to_string())
}

pub fn t_args(id: &str, args: &[(&str, &str)]) -> String {
    let template = t(id);
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{'
            && let Some(end) = template[i..].find('}') {
                let placeable = template[i + 1..i + end].trim();
                if let Some(name) = placeable.strip_prefix('$') {
                    if let Some((_, val)) = args.iter().find(|(k, _)| *k == name) {
                        out.push_str(val);
                        i += end + 1;
                        continue;
                    }
                    i += end + 1;
                    continue;
                }
            }
        if let Some(ch) = template[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    out
}
