//! Bringing files into the Drive: a small file as one multipart request, a
//! large one as a resumable upload whose parts are verified against their
//! digests and then imported by its blob.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use reqwest::Method;
use sp_protocol::ids::{ContextId, DocumentUploadId};
use sp_protocol::wire::drive::{
    DriveBlob, DriveCreated, DriveUpload, DriveUploadPart, INLINE_UPLOAD_LIMIT, ImportBlobRequest,
    OpenUploadRequest,
};
use sp_protocol::wire::paths;
use tokio_util::sync::CancellationToken;

use super::types::ImportFile;
use super::{DriveClient, EmptyBody, TRANSFER_TIMEOUT};
use crate::error::DriveError;
use crate::hash::sha256_hex;
use crate::http::{Call, FormPart, Payload};
use crate::upload::{UploadProgress, already_sent, plan_parts};

const CREATE: &str = "POST /api/v1/documents";
const IMPORT: &str = "POST /api/v1/documents/import";
const OPEN_UPLOAD: &str = "POST /api/v1/uploads";
const UPLOAD_STATUS: &str = "GET /api/v1/uploads/{id}";
const UPLOAD_PART: &str = "PUT /api/v1/uploads/{id}/parts/{n}";
const UPLOAD_COMPLETE: &str = "POST /api/v1/uploads/{id}/complete";

impl DriveClient {
    pub async fn import_blob(
        &self,
        request: &ImportBlobRequest,
        context_id: Option<&ContextId>,
    ) -> Result<DriveCreated, DriveError> {
        let call = Call::new(
            Method::POST,
            self.url(paths::DOCUMENTS, &["import"])?,
            IMPORT,
        )
        .json(request)?
        .context(context_id);
        self.json(call, &[201]).await
    }

    pub async fn import_file(
        &self,
        file: ImportFile,
        progress: &(dyn Fn(UploadProgress) + Send + Sync),
        cancel: &CancellationToken,
    ) -> Result<DriveCreated, DriveError> {
        let mime = file
            .mime
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_owned());
        if file.bytes.len() > INLINE_UPLOAD_LIMIT {
            let opened = self.open_upload(&file.name, &mime, &file.bytes).await?;
            let blob = self
                .finish_upload(&opened, &file.bytes, progress, cancel)
                .await?;
            let request = ImportBlobRequest {
                blob_ref: blob.blob_ref,
                name: file.name,
                folder_id: file.folder_id,
                message: file.message,
            };
            return self.import_blob(&request, file.context_id.as_ref()).await;
        }
        let total = u64::try_from(file.bytes.len()).unwrap_or(u64::MAX);
        let mut parts = vec![
            FormPart::File {
                name: "file",
                bytes: file.bytes.clone(),
                file_name: file.name.clone(),
                mime,
            },
            FormPart::Text {
                name: "name",
                value: file.name.clone(),
                mime: None,
            },
        ];
        if let Some(folder) = &file.folder_id {
            parts.push(FormPart::Text {
                name: "folder_id",
                value: folder.to_string(),
                mime: None,
            });
        }
        if let Some(message) = &file.message {
            parts.push(FormPart::Text {
                name: "message",
                value: message.clone(),
                mime: None,
            });
        }
        let call = Call::new(Method::POST, self.url(paths::DOCUMENTS, &[])?, CREATE)
            .payload(Payload::Multipart(parts))
            .context(file.context_id.as_ref())
            .limits(TRANSFER_TIMEOUT, crate::http::SMALL_BODY);
        let created = tokio::select! {
            () = cancel.cancelled() => return Err(DriveError::Cancelled),
            created = self.json::<DriveCreated>(call, &[201]) => created?,
        };
        progress(UploadProgress {
            sent_bytes: total,
            total_bytes: total,
            parts_sent: 1,
            parts_total: 1,
        });
        Ok(created)
    }

    pub async fn open_upload(
        &self,
        name: &str,
        mime: &str,
        bytes: &[u8],
    ) -> Result<DriveUpload, DriveError> {
        let request = OpenUploadRequest {
            name: name.to_owned(),
            mime: mime.to_owned(),
            size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            sha256: sha256_hex(bytes),
        };
        let call =
            Call::new(Method::POST, self.url(paths::UPLOADS, &[])?, OPEN_UPLOAD).json(&request)?;
        self.json(call, &[201]).await
    }

    pub async fn upload_status(
        &self,
        upload: &DocumentUploadId,
    ) -> Result<DriveUpload, DriveError> {
        let id = upload.to_string();
        let call = Call::new(
            Method::GET,
            self.url(paths::UPLOADS, &[id.as_str()])?,
            UPLOAD_STATUS,
        );
        self.json(call, &[200]).await
    }

    pub async fn put_part(
        &self,
        upload: &DocumentUploadId,
        number: u32,
        bytes: Bytes,
    ) -> Result<DriveUploadPart, DriveError> {
        if number == 0 {
            return Err(DriveError::InvalidInput {
                what: "upload parts are numbered from 1",
            });
        }
        let id = upload.to_string();
        let part = number.to_string();
        let expected = sha256_hex(&bytes);
        let url = self.url(paths::UPLOADS, &[id.as_str(), "parts", part.as_str()])?;
        let call = Call::new(Method::PUT, url, UPLOAD_PART)
            .payload(Payload::Raw(bytes))
            .limits(TRANSFER_TIMEOUT, crate::http::SMALL_BODY);
        let stored: DriveUploadPart = self.json(call, &[200]).await?;
        if stored.part_number != number || !stored.sha256.eq_ignore_ascii_case(&expected) {
            return Err(DriveError::PartAltered {
                endpoint: UPLOAD_PART,
                part: number,
            });
        }
        Ok(stored)
    }

    pub async fn complete_upload(
        &self,
        upload: &DocumentUploadId,
    ) -> Result<DriveBlob, DriveError> {
        let id = upload.to_string();
        let url = self.url(paths::UPLOADS, &[id.as_str(), "complete"])?;
        let call = Call::new(Method::POST, url, UPLOAD_COMPLETE)
            .json(&EmptyBody {})?
            .limits(TRANSFER_TIMEOUT, crate::http::SMALL_BODY);
        self.json(call, &[200]).await
    }

    pub async fn finish_upload(
        &self,
        upload: &DriveUpload,
        bytes: &Bytes,
        progress: &(dyn Fn(UploadProgress) + Send + Sync),
        cancel: &CancellationToken,
    ) -> Result<DriveBlob, DriveError> {
        let plan = plan_parts(upload, bytes.len())?;
        let mut sent = already_sent(upload, &plan);
        let parts_total = upload.part_count;
        let mut parts_sent =
            parts_total.saturating_sub(u32::try_from(plan.len()).unwrap_or(u32::MAX));
        progress(UploadProgress {
            sent_bytes: sent,
            total_bytes: upload.size,
            parts_sent,
            parts_total,
        });
        for part in plan {
            if cancel.is_cancelled() {
                return Err(DriveError::Cancelled);
            }
            let chunk = bytes.slice(part.range.clone());
            let length = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
            tokio::select! {
                () = cancel.cancelled() => return Err(DriveError::Cancelled),
                stored = self.put_part(&upload.upload_id, part.number, chunk) => { stored?; },
            }
            sent = sent.saturating_add(length);
            parts_sent = parts_sent.saturating_add(1);
            progress(UploadProgress {
                sent_bytes: sent,
                total_bytes: upload.size,
                parts_sent,
                parts_total,
            });
        }
        if cancel.is_cancelled() {
            return Err(DriveError::Cancelled);
        }
        self.complete_upload(&upload.upload_id).await
    }
}
