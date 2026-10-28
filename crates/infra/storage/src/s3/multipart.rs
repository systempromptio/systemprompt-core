//! Writing one S3 object from a stream with a multipart upload.
//!
//! A stream that fits in one chunk is a single `PUT`. Anything longer
//! opens a multipart upload (`POST ?uploads`), sends each chunk as a part
//! (`PUT ?partNumber=n&uploadId=…`) and completes it with the parts' ETags
//! (`POST ?uploadId=…`). Only the chunk being sent is in memory. Any
//! failure aborts the upload (`DELETE ?uploadId=…`) so no parts are left
//! billed in the bucket. S3 can answer a complete with `200` and an
//! `<Error>` body, which is treated as a failure.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::Method;
use reqwest::header::ETAG;

use super::client::Call;
use super::{S3Error, S3FileStorage};
use crate::chunks::Lookahead;

fn tag_value<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml[start..].find(&format!("</{tag}>"))?;
    Some(&xml[start..end])
}

fn upload_query(upload_id: &str) -> String {
    format!("uploadId={}", urlencoding::encode(upload_id))
}

fn complete_body(etags: &[String]) -> String {
    let parts: String = etags
        .iter()
        .zip(1..)
        .map(|(etag, n)| format!("<Part><PartNumber>{n}</PartNumber><ETag>{etag}</ETag></Part>"))
        .collect();
    format!("<CompleteMultipartUpload>{parts}</CompleteMultipartUpload>")
}

impl S3FileStorage {
    pub(crate) async fn put_stream(
        &self,
        name: &str,
        content_type: &str,
        mut chunks: Lookahead,
    ) -> Result<(), StreamPutError> {
        let Some((first, last)) = chunks.next().await? else {
            return Ok(self.put(name, content_type, &[]).await?);
        };
        if last {
            return Ok(self.put(name, content_type, &first).await?);
        }
        let upload_id = self.create_multipart(name, content_type).await?;
        match self.send_parts(name, &upload_id, first, &mut chunks).await {
            Ok(etags) => match self.complete_multipart(name, &upload_id, &etags).await {
                Ok(()) => Ok(()),
                Err(error) => Err(self.abandon(name, &upload_id, error.into()).await),
            },
            Err(error) => Err(self.abandon(name, &upload_id, error).await),
        }
    }

    async fn send_parts(
        &self,
        name: &str,
        upload_id: &str,
        first: Vec<u8>,
        chunks: &mut Lookahead,
    ) -> Result<Vec<String>, StreamPutError> {
        let mut etags = vec![self.upload_part(name, upload_id, 1, &first).await?];
        while let Some((part, _)) = chunks.next().await? {
            let number = etags.len() + 1;
            etags.push(self.upload_part(name, upload_id, number, &part).await?);
        }
        Ok(etags)
    }

    async fn create_multipart(&self, name: &str, content_type: &str) -> Result<String, S3Error> {
        let response = self
            .call(Call {
                method: Method::POST,
                name,
                query: "uploads=",
                content_type: Some(content_type),
                payload: &[],
                range: None,
            })
            .await?;
        let body = response.text().await?;
        tag_value(&body, "UploadId")
            .map(str::to_owned)
            .ok_or(S3Error::ObjectHeader {
                name: name.to_owned(),
                header: "UploadId",
            })
    }

    async fn upload_part(
        &self,
        name: &str,
        upload_id: &str,
        number: usize,
        payload: &[u8],
    ) -> Result<String, S3Error> {
        let query = format!("partNumber={number}&{}", upload_query(upload_id));
        let response = self
            .call(Call {
                method: Method::PUT,
                name,
                query: &query,
                content_type: None,
                payload,
                range: None,
            })
            .await?;
        response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .ok_or(S3Error::ObjectHeader {
                name: name.to_owned(),
                header: "etag",
            })
    }

    async fn complete_multipart(
        &self,
        name: &str,
        upload_id: &str,
        etags: &[String],
    ) -> Result<(), S3Error> {
        let body = complete_body(etags);
        let response = self
            .call(Call {
                method: Method::POST,
                name,
                query: &upload_query(upload_id),
                content_type: Some("application/xml"),
                payload: body.as_bytes(),
                range: None,
            })
            .await?;
        let answer = response.text().await?;
        if answer.contains("<Error>") {
            return Err(S3Error::Status {
                status: 200,
                body: answer.chars().take(512).collect(),
            });
        }
        Ok(())
    }

    async fn abandon(&self, name: &str, upload_id: &str, error: StreamPutError) -> StreamPutError {
        let aborted = self
            .call(Call {
                method: Method::DELETE,
                name,
                query: &upload_query(upload_id),
                content_type: None,
                payload: &[],
                range: None,
            })
            .await;
        if let Err(abort) = aborted {
            tracing::warn!(object = name, error = %abort, "Failed to abort an S3 multipart upload");
        }
        error
    }
}

/// Why a streamed write failed: the stream itself, or S3.
#[derive(Debug, thiserror::Error)]
pub(crate) enum StreamPutError {
    #[error(transparent)]
    Source(#[from] systemprompt_traits::FileStorageError),
    #[error(transparent)]
    S3(#[from] S3Error),
}
