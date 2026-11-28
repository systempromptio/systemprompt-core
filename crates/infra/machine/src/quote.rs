//! Text handed to another program's parser: an XML document, a Windows
//! command line, a POSIX shell line and an AppleScript string. Each function
//! is free of platform conditions so every system's tests cover the exact text
//! the other systems hand over.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::iter::repeat_n;
use std::path::Path;

pub(crate) fn is_plain(value: &str) -> bool {
    !value.chars().any(char::is_control)
}

pub(crate) fn program_text(program: &Path) -> Option<&str> {
    program
        .to_str()
        .filter(|text| program.is_absolute() && is_plain(text))
}

pub(crate) fn xml_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

// Why: the Windows C runtime splits a command line on unquoted blanks, reads a
// run of backslashes literally unless a quote follows it, and then halves the
// run; an argument is quoted and its backslashes doubled to survive that.
fn windows_word(argument: &str) -> String {
    if !argument.is_empty() && !argument.chars().any(|c| matches!(c, ' ' | '\t' | '"')) {
        return argument.to_owned();
    }
    let mut out = String::from('"');
    let mut backslashes = 0usize;
    for c in argument.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            },
            other => {
                out.extend(repeat_n('\\', backslashes));
                out.push(other);
                backslashes = 0;
            },
        }
    }
    out.extend(repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

pub(crate) fn windows_command_line(arguments: &[String]) -> String {
    arguments
        .iter()
        .map(|argument| windows_word(argument))
        .collect::<Vec<_>>()
        .join(" ")
}

fn posix_word(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

pub(crate) fn posix_command_line(program: &str, arguments: &[String]) -> String {
    std::iter::once(program)
        .chain(arguments.iter().map(String::as_str))
        .map(posix_word)
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn applescript_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str(r#"\""#),
            other => out.push(other),
        }
    }
    out
}
