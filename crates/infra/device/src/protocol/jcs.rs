//! RFC 8785 JSON Canonicalization Scheme.
//!
//! The scheme is ECMAScript's own JSON serialization with object members
//! sorted: numbers print as `Number.prototype.toString` prints an IEEE-754
//! double, strings escape exactly what `JSON.stringify` escapes, and member
//! names sort by their UTF-16 code units. A manifest payload is signed in this
//! form, so re-canonicalizing a parsed payload gives back the signed string.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use serde_json::{Map, Number, Value};

// JSON: canonicalization is defined over any JSON document, so the input is
// untyped.
#[must_use]
pub fn canonicalize(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value);
    out
}

pub fn to_canonical<T: Serialize + ?Sized>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_value(value).map(|value| canonicalize(&value))
}

pub fn canonicalize_str(json: &str) -> Result<String, serde_json::Error> {
    serde_json::from_str::<Value>(json).map(|value| canonicalize(&value))
}

#[must_use]
pub fn is_canonical(json: &str) -> bool {
    canonicalize_str(json).is_ok_and(|canonical| canonical == json)
}

// JSON: walks an arbitrary JSON document, member by member.
fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => write_number(out, number),
        Value::String(text) => write_string(out, text),
        Value::Array(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        },
        Value::Object(members) => write_object(out, members),
    }
}

// JSON: an object of an arbitrary JSON document, written with its members
// sorted.
fn write_object(out: &mut String, members: &Map<String, Value>) {
    let mut sorted: Vec<(&String, &Value)> = members.iter().collect();
    sorted.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    out.push('{');
    for (index, (name, value)) in sorted.into_iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write_string(out, name);
        out.push(':');
        write_value(out, value);
    }
    out.push('}');
}

fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let code = u32::from(c);
                out.push_str("\\u00");
                out.push(hex_digit(code >> 4));
                out.push(hex_digit(code & 0x0f));
            },
            c => out.push(c),
        }
    }
    out.push('"');
}

fn hex_digit(nibble: u32) -> char {
    char::from_digit(nibble, 16).unwrap_or('0')
}

// Why: JSON numbers are IEEE-754 doubles to ECMAScript, so every number (an
// integer too) prints the way `Number.prototype.toString` prints the double
// nearest to it.
fn write_number(out: &mut String, number: &Number) {
    match number.as_f64() {
        Some(value) => write_double(out, value),
        None => out.push_str(&number.to_string()),
    }
}

fn write_double(out: &mut String, value: f64) {
    if value == 0.0 {
        out.push('0');
        return;
    }
    if value.is_sign_negative() {
        out.push('-');
    }
    // Why: RFC 8785 takes ECMAScript's shortest round-trip digits; Rust's `{:e}`
    // prints them.
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific
        .split_once('e')
        .unwrap_or((scientific.as_str(), "0"));
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let k = i32::try_from(digits.len()).unwrap_or(i32::MAX);
    let n = exponent + 1;
    if k <= n && n <= 21 {
        out.push_str(&digits);
        out.extend(std::iter::repeat_n(
            '0',
            usize::try_from(n - k).unwrap_or(0),
        ));
    } else if 0 < n && n <= 21 {
        let split = usize::try_from(n).unwrap_or(0);
        out.push_str(&digits[..split]);
        out.push('.');
        out.push_str(&digits[split..]);
    } else if -6 < n && n <= 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', usize::try_from(-n).unwrap_or(0)));
        out.push_str(&digits);
    } else {
        let e = n - 1;
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if e < 0 { '-' } else { '+' });
        out.push_str(&e.unsigned_abs().to_string());
    }
}
