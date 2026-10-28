//! Writing one Cloud Storage object from a stream with a resumable upload.
//!
//! A stream that fits in one chunk is a single media upload. Anything
//! longer opens a resumable session (`uploadType=resumable`, whose
//! `Location` is the session URI) and `PUT`s each chunk to it with a
//! `Content-Range`; every chunk but the last is a multiple of 256 KiB and
//! is answered `308`, and the last names the total size. Only the chunk
//! being sent is in memory. A failure cancels the session with `DELETE`, and
//! so does a write cancelled mid-way, from a drop guard; a session nobody
//! cancels expires on its own after a week.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use reqwest::StatusCode;
use reqwest::header::{CONTENT_RANGE, LOCATION};
use url::Url;

use super::{GcsError, GcsFileStorage};
use crate::chunks::Lookahead;

const RESUME_INCOMPLETE: u16 = 308;
const UPLOAD_CONTENT_TYPE: &str = "x-upload-content-type";

impl GcsFileStorage {
    pub(crate) async fn upload_stream(
        &self,
        name: &str,
        content_type: &str,
        mut chunks: Lookahead,
    ) -> Result<(), StreamUploadError> {
        let Some((first, last)) = chunks.next().await? else {
            return Ok(self.upload(name, content_type, &[]).await?);
        };
        if last {
            return Ok(self.upload(name, content_type, &first).await?);
        }
        let session = self.open_session(name, content_type).await?;
        let mut guard = CancelGuard {
            http: Some(self.http.clone()),
            session: session.clone(),
        };
        let result = match self.send_chunks(&session, first, &mut chunks).await {
            Ok(()) => Ok(()),
            Err(error) => {
                if let Err(cancel) = self.http.delete(session.clone()).send().await {
                    tracing::warn!(object = name, error = %cancel, "Failed to cancel a resumable upload");
                }
                Err(error)
            },
        };
        guard.http = None;
        result
    }

    async fn open_session(&self, name: &str, content_type: &str) -> Result<Url, GcsError> {
        let mut url = self.params.upload_base.join(&format!(
            "upload/storage/v1/b/{}/o",
            urlencoding::encode(&self.params.bucket)
        ))?;
        url.query_pairs_mut()
            .append_pair("uploadType", "resumable")
            .append_pair("name", name);
        let bearer = self.tokens.bearer().await?;
        let response = self
            .http
            .post(url)
            .bearer_auth(bearer)
            .header(UPLOAD_CONTENT_TYPE, content_type)
            .header(reqwest::header::CONTENT_LENGTH, 0)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(GcsError::Status {
                status: response.status().as_u16(),
                body: crate::error_body::capped_error_body(response).await?,
            });
        }
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(GcsError::ObjectField {
                name: name.to_owned(),
                field: "resumable session location",
            })?;
        Ok(Url::parse(location)?)
    }

    async fn send_chunks(
        &self,
        session: &Url,
        first: Vec<u8>,
        chunks: &mut Lookahead,
    ) -> Result<(), StreamUploadError> {
        let mut offset: u64 = 0;
        let mut current = Some((first, false));
        while let Some((chunk, last)) = current.take() {
            let end = offset + chunk.len() as u64;
            let total = if last {
                end.to_string()
            } else {
                "*".to_owned()
            };
            let range = format!("bytes {offset}-{}/{total}", end - 1);
            let response = self
                .http
                .put(session.clone())
                .header(CONTENT_RANGE, range)
                .body(chunk)
                .send()
                .await
                .map_err(GcsError::from)?;
            let status = response.status();
            let accepted = if last {
                status.is_success()
            } else {
                status.as_u16() == RESUME_INCOMPLETE
            };
            if !accepted {
                let body = crate::error_body::capped_error_body(response)
                    .await
                    .map_err(GcsError::from)?;
                return Err(GcsError::Status {
                    status: status.as_u16(),
                    body,
                }
                .into());
            }
            offset = end;
            if !last {
                current = Some(chunks.next().await?.ok_or(GcsError::Status {
                    status: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
                    body: "the stream ended before its last chunk".to_owned(),
                })?);
            }
        }
        Ok(())
    }
}

/// Cancels a resumable session whose write was dropped mid-way (the request
/// future was cancelled). The write disarms it once it has finished or
/// cancelled the session itself.
struct CancelGuard {
    http: Option<reqwest::Client>,
    session: Url,
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        let Some(http) = self.http.take() else {
            return;
        };
        let session = self.session.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                if let Err(cancel) = http.delete(session).send().await {
                    tracing::warn!(error = %cancel, "Failed to cancel a dropped resumable upload");
                }
            });
        }
    }
}

/// Why a streamed write failed: the stream itself, or Cloud Storage.
#[derive(Debug, thiserror::Error)]
pub(crate) enum StreamUploadError {
    #[error(transparent)]
    Source(#[from] systemprompt_traits::FileStorageError),
    #[error(transparent)]
    Gcs(#[from] GcsError),
}
