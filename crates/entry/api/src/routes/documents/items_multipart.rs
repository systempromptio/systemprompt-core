//! The `multipart/form-data` body of `POST /`: the `file` part, with its
//! optional `name`, `folder_id` and `format` parts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Multipart;
use systemprompt_identifiers::DocumentFolderId;
use systemprompt_models::api::ApiError;
use systemprompt_models::documents::DocumentFormat;

use super::error::DocumentsResult;
use super::items::folder_id;

const OPAQUE_MIME: &str = "application/octet-stream";

pub(super) struct MultipartFile {
    pub(super) name: String,
    pub(super) folder: Option<DocumentFolderId>,
    pub(super) format: Option<DocumentFormat>,
    pub(super) mime: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) async fn read_file(mut form: Multipart) -> DocumentsResult<MultipartFile> {
    let mut name = None;
    let mut folder = None;
    let mut format = None;
    let mut file: Option<(Option<String>, Vec<u8>)> = None;
    let mut mime = None;
    while let Some(field) = form.next_field().await? {
        let part = field.name().unwrap_or_default().to_owned();
        let file_name = field.file_name().map(str::to_owned);
        if part == "file" {
            mime = field.content_type().map(str::to_owned);
        }
        let bytes = field.bytes().await?;
        let text = || String::from_utf8_lossy(&bytes).trim().to_owned();
        match part.as_str() {
            "file" => file = Some((file_name, bytes.to_vec())),
            "name" => name = Some(text()),
            "folder_id" => folder = folder_id(Some(&text()))?,
            "format" => {
                format = Some(text().parse::<DocumentFormat>().map_err(|source| {
                    ApiError::bad_request("format is not a known document format")
                        .with_source(source)
                })?);
            },
            _ => {},
        }
    }
    let (file_name, bytes) =
        file.ok_or_else(|| ApiError::bad_request("multipart import needs a `file` part"))?;
    let name = name
        .or(file_name)
        .ok_or_else(|| ApiError::bad_request("multipart import needs a `name`"))?;
    Ok(MultipartFile {
        name,
        folder,
        format,
        mime: mime.unwrap_or_else(|| OPAQUE_MIME.to_owned()),
        bytes,
    })
}
