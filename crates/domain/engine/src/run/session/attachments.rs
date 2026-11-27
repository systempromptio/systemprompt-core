//! Turning prompt attachments into message parts, and data URLs back into
//! bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use crate::store::PartBody;

use super::AttachmentInput;
use crate::run::error::EngineError;

const OCTET_STREAM: &str = "application/octet-stream";

pub(crate) async fn parts(
    inputs: &[AttachmentInput],
    directory: &Path,
) -> Result<Vec<PartBody>, EngineError> {
    let mut parts = Vec::new();
    for input in inputs {
        match input {
            AttachmentInput::Path(given) => {
                let path = if given.is_absolute() {
                    given.clone()
                } else {
                    directory.join(given)
                };
                parts.extend(file_parts(&path).await?);
            },
            AttachmentInput::DataUrl { url, name } => {
                let (mime, _) = parse_data_url(url).ok_or(EngineError::InvalidDataUrl)?;
                parts.push(PartBody::File {
                    mime,
                    name: name.clone(),
                    url: url.clone(),
                });
            },
        }
    }
    Ok(parts)
}

async fn file_parts(path: &Path) -> Result<Vec<PartBody>, EngineError> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|source| EngineError::Attachment {
            path: path.to_path_buf(),
            source,
        })?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let mime = mime_for(path);
    if mime.starts_with("image/") || mime == "application/pdf" {
        let url = data_url(mime, &bytes);
        return Ok(vec![PartBody::File {
            mime: mime.to_owned(),
            name,
            url,
        }]);
    }
    match String::from_utf8(bytes) {
        Ok(text) => {
            let text = format!("Attached file {}:\n\n{text}", slash(path));
            Ok(vec![
                PartBody::File {
                    mime: mime.to_owned(),
                    name,
                    url: file_url(path),
                },
                PartBody::Text {
                    text,
                    synthetic: true,
                },
            ])
        },
        Err(error) => {
            let url = data_url(OCTET_STREAM, error.as_bytes());
            Ok(vec![PartBody::File {
                mime: OCTET_STREAM.to_owned(),
                name,
                url,
            }])
        },
    }
}

pub(crate) fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

pub(crate) fn parse_data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (header, data) = rest.split_once(',')?;
    let mime = header.strip_suffix(";base64")?;
    let mime = if mime.is_empty() { OCTET_STREAM } else { mime };
    Some((mime.to_owned(), data.to_owned()))
}

pub(crate) fn decode(data: &str) -> Option<Vec<u8>> {
    STANDARD.decode(data.trim()).ok()
}

fn file_url(path: &Path) -> String {
    format!("file:///{}", slash(path).trim_start_matches('/'))
}

pub(crate) fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn mime_for(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "md" | "markdown" => "text/markdown",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        _ => "text/plain",
    }
}
