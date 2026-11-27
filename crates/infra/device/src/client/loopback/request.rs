//! Reading and judging the one request line a loopback connection sends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::protocol::OneTimeCode;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

pub(super) const CALLBACK_PATH: &str = "/callback";

const REQUEST_HEAD_LIMIT: usize = 8 * 1024;
const CODE_MAX_LEN: usize = 512;

#[derive(Debug)]
pub(super) enum Verdict {
    Code(OneTimeCode),
    Denied,
    Invalid,
}

pub(super) async fn read_request_line(stream: &mut TcpStream) -> Option<String> {
    let mut head = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            break;
        }
        head.extend_from_slice(chunk.get(..read)?);
        if head.windows(4).any(|window| window == b"\r\n\r\n") || head.len() >= REQUEST_HEAD_LIMIT {
            break;
        }
    }
    let end = head.iter().position(|&b| b == b'\n')?;
    let line = std::str::from_utf8(head.get(..end)?).ok()?;
    Some(line.trim_end().to_owned())
}

pub(super) fn judge(line: &str, expected_state: &str) -> Verdict {
    let mut parts = line.split(' ');
    let (Some("GET"), Some(target)) = (parts.next(), parts.next()) else {
        return Verdict::Invalid;
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != CALLBACK_PATH {
        return Verdict::Invalid;
    }
    let mut state = None;
    let mut code = None;
    let mut error = None;
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        let slot = match &*key {
            "state" => &mut state,
            "code" => &mut code,
            "error" => &mut error,
            _ => continue,
        };
        if slot.replace(value.into_owned()).is_some() {
            return Verdict::Invalid;
        }
    }
    let Some(state) = state else {
        return Verdict::Invalid;
    };
    if !same_secret(state.as_bytes(), expected_state.as_bytes()) {
        return Verdict::Invalid;
    }
    match (code, error) {
        (None, Some(error)) if error == "denied" => Verdict::Denied,
        (Some(code), None) if plausible_code(&code) => {
            OneTimeCode::new(code).map_or(Verdict::Invalid, Verdict::Code)
        },
        _ => Verdict::Invalid,
    }
}

fn plausible_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= CODE_MAX_LEN
        && code
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
}

fn same_secret(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
