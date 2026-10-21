//! Bounded read of an object store's error response body.
//!
//! A failing request's body ends up in an error's `Display` and so in logs;
//! a proxy or misconfigured endpoint can answer with an arbitrarily large
//! page. At most [`ERROR_BODY_LIMIT`] bytes are read and the rest of the
//! response is dropped unread.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::Response;

pub(crate) const ERROR_BODY_LIMIT: usize = 2048;

const TRUNCATED: &str = " [truncated]";

pub(crate) async fn capped_error_body(mut response: Response) -> Result<String, reqwest::Error> {
    let mut buf: Vec<u8> = Vec::new();
    let mut truncated = false;
    while let Some(chunk) = response.chunk().await? {
        let room = ERROR_BODY_LIMIT - buf.len();
        if chunk.len() > room {
            buf.extend(chunk.iter().take(room));
            truncated = true;
            break;
        }
        buf.extend_from_slice(&chunk);
    }
    let mut body = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        body.push_str(TRUNCATED);
    }
    Ok(body)
}
