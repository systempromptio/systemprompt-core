//! Recovers the one-time sign-in code from whatever the user pasted: a bare
//! code, a full `--code` invocation, or terminal output around either.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub fn extract_code(pasted: &str) -> Result<String, String> {
    let pasted = strip_terminal_noise(pasted);
    let pasted = pasted.trim();
    if pasted.is_empty() {
        return Err("nothing pasted".into());
    }

    if let Some(code) = code_after_flag(pasted) {
        return Ok(code);
    }

    let Some((_, query)) = pasted.split_once('?') else {
        if pasted.split_whitespace().count() > 1 {
            return Err(
                "that looks like a command but carries no `--code` — paste just the code, or the \
                 whole command the page displayed"
                    .into(),
            );
        }
        return Ok(pasted.to_owned());
    };
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("code=") {
            let code = value.split('#').next().unwrap_or(value);
            if !code.is_empty() {
                return Ok(code.to_owned());
            }
        }
        if let Some(reason) = pair.strip_prefix("error=") {
            return Err(format!("the sign-in was not approved ({reason})"));
        }
    }
    Err("that URL carries no `code` parameter — paste the code the page displayed".into())
}

// Why: raw stdin retains terminal bracketed-paste escapes (ESC[200~ /
// ESC[201~).
pub fn strip_terminal_noise(pasted: &str) -> String {
    let mut out = String::with_capacity(pasted.len());
    let mut chars = pasted.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.next() == Some('[') {
                for tail in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&tail) {
                        break;
                    }
                }
            }
            continue;
        }
        if !c.is_control() || c.is_whitespace() {
            out.push(c);
        }
    }
    out
}

pub fn code_after_flag(pasted: &str) -> Option<String> {
    let mut tokens = pasted.split_whitespace();
    while let Some(token) = tokens.next() {
        if let Some(code) = token.strip_prefix("--code=") {
            return (!code.is_empty()).then(|| code.to_owned());
        }
        if token == "--code" {
            return tokens.next().map(str::to_owned).filter(|c| !c.is_empty());
        }
    }
    None
}
