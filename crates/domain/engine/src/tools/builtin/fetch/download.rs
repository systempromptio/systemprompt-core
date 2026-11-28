//! The bounded GET behind the fetch tool: a redirect handed back to the
//! caller unfollowed, the declared length checked first, the body read chunk
//! by chunk up to the byte limit, and a failed status reported with the start
//! of its body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use reqwest::header::{ACCEPT, CONTENT_TYPE, LOCATION};
use url::Url;

use crate::tools::error::ToolError;

#[derive(Debug)]
pub(super) struct Fetched {
    pub(super) final_url: String,
    pub(super) content_type: String,
    pub(super) body: Vec<u8>,
}

#[derive(Debug)]
pub(super) enum Reply {
    Page(Fetched),
    Redirect(Url),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Request<'a> {
    pub(super) url: &'a Url,
    pub(super) accept: &'a str,
    pub(super) timeout: Duration,
    pub(super) max_bytes: usize,
}

pub(super) async fn download(
    client: &reqwest::Client,
    request: Request<'_>,
) -> Result<Reply, ToolError> {
    let Request {
        url,
        accept,
        timeout,
        max_bytes,
    } = request;
    let mut response = client
        .get(url.clone())
        .header(ACCEPT, accept)
        .timeout(timeout)
        .send()
        .await
        .map_err(|err| ToolError::caused(format!("could not fetch {url}"), err))?;
    let status = response.status();
    if status.is_redirection() {
        return redirect_target(url, response.headers().get(LOCATION)).map(Reply::Redirect);
    }
    let final_url = response.url().to_string();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    if let Some(length) = response.content_length()
        && length > max_bytes as u64
    {
        return Err(ToolError::invalid(format!(
            "{final_url} is {length} bytes, over the {max_bytes} byte limit"
        )));
    }
    let mut body: Vec<u8> = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| ToolError::caused(format!("reading {final_url} failed"), err))?
    {
        if body.len() + chunk.len() > max_bytes {
            return Err(ToolError::invalid(format!(
                "{final_url} is larger than the {max_bytes} byte limit"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        let snippet: String = String::from_utf8_lossy(&body).chars().take(300).collect();
        return Err(ToolError::failed(format!(
            "{final_url} answered {status}: {}",
            snippet.trim()
        )));
    }
    Ok(Reply::Page(Fetched {
        final_url,
        content_type,
        body,
    }))
}

fn redirect_target(
    url: &Url,
    location: Option<&reqwest::header::HeaderValue>,
) -> Result<Url, ToolError> {
    let location = location
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ToolError::failed(format!("{url} redirects without a location")))?;
    url.join(location)
        .map_err(|err| ToolError::malformed(format!("{url} redirects to an invalid URL"), err))
}
