//! Request payloads: none, JSON, raw bytes or a multipart form, and the
//! multipart form built from its parts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use reqwest::multipart::{Form, Part};
use sp_client::ClientError;

#[derive(Debug, Clone)]
pub(crate) enum Payload {
    Empty,
    Json(Bytes),
    Raw(Bytes),
    Multipart(Vec<FormPart>),
}

#[derive(Debug, Clone)]
pub(crate) enum FormPart {
    Text {
        name: &'static str,
        value: String,
        mime: Option<&'static str>,
    },
    File {
        name: &'static str,
        bytes: Bytes,
        file_name: String,
        mime: String,
    },
}

pub(super) fn form(parts: &[FormPart]) -> Result<Form, ClientError> {
    let invalid = |_invalid: reqwest::Error| ClientError::InvalidInput {
        what: "a form part's media type is not valid",
    };
    let mut form = Form::new();
    for part in parts {
        form = match part {
            FormPart::Text {
                name,
                value,
                mime: None,
            } => form.text(*name, value.clone()),
            FormPart::Text {
                name,
                value,
                mime: Some(mime),
            } => form.part(
                *name,
                Part::text(value.clone()).mime_str(mime).map_err(invalid)?,
            ),
            FormPart::File {
                name,
                bytes,
                file_name,
                mime,
            } => {
                let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                let file = Part::stream_with_length(bytes.clone(), length)
                    .file_name(file_name.clone())
                    .mime_str(mime)
                    .map_err(invalid)?;
                form.part(*name, file)
            },
        };
    }
    Ok(form)
}
