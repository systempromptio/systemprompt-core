//! Shared steps of the document service: codec lookup, blocking codec
//! calls, name rules, blob reads, event recipients and paths.
//!
//! Event recipients are the owner and every present user who still holds
//! `view` or better when the event is emitted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::sync::Arc;

use systemprompt_identifiers::{DocumentFolderId, UserId};
use systemprompt_models::documents::DocumentFormat;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, EditOp, resolve_targets,
};

use super::DocumentService;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision};

const MAX_NAME_CHARS: usize = 255;

pub(super) async fn blocking<T, F>(work: F) -> Result<T, DocumentsError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, DocumentsError> + Send + 'static,
{
    tokio::task::spawn_blocking(work).await?
}

pub(super) fn validate_name(name: &str) -> Result<(), DocumentsError> {
    let invalid = name.is_empty()
        || name.chars().count() > MAX_NAME_CHARS
        || name == "."
        || name == ".."
        || name.trim() != name
        || name
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control());
    if invalid {
        return Err(DocumentsError::InvalidName {
            name: name.to_owned(),
        });
    }
    Ok(())
}

pub(super) fn document_name(name: &str, format: DocumentFormat) -> Result<String, DocumentsError> {
    validate_name(name)?;
    let has_extension = name
        .rsplit_once('.')
        .and_then(|(_, ext)| DocumentFormat::from_extension(ext))
        == Some(format);
    let named = if has_extension {
        name.to_owned()
    } else {
        format!("{name}.{}", format.extension())
    };
    validate_name(&named)?;
    Ok(named)
}

pub(super) fn apply_with(
    codec: &dyn DocumentCodec,
    format: DocumentFormat,
    bytes: &[u8],
    ops: Vec<EditOp>,
) -> Result<Vec<u8>, DocumentsError> {
    let projection = codec.project(bytes)?;
    let resolved = resolve_targets(&projection, ops)?;
    codec
        .apply(bytes, &projection, &resolved.ops)
        .map_err(|error| match error {
            CodecError::UnsupportedOp { op, format: _ } => DocumentsError::UnsupportedOp {
                op,
                format,
                accepted_kinds: codec.insert_kinds(format),
            },
            other => other.into(),
        })
}

pub(super) fn format_of_name(name: &str) -> Option<DocumentFormat> {
    name.rsplit_once('.')
        .and_then(|(_, ext)| DocumentFormat::from_extension(ext))
}

impl DocumentService {
    pub(super) fn codec(
        &self,
        format: DocumentFormat,
    ) -> Result<Arc<dyn DocumentCodec>, DocumentsError> {
        self.codecs
            .for_format(format)
            .ok_or(DocumentsError::UnsupportedFormat { format })
    }

    #[must_use]
    pub fn insert_kinds(&self, format: DocumentFormat) -> &'static [&'static str] {
        self.codecs
            .for_format(format)
            .map_or(&[], |codec| codec.insert_kinds(format))
    }

    #[must_use]
    pub fn formats(&self) -> Vec<DocumentFormat> {
        self.codecs.formats()
    }

    pub(super) const fn check_size(&self, size: usize) -> Result<(), DocumentsError> {
        let size = size as u64;
        let limit = self.config.max_upload_bytes;
        if size > limit {
            return Err(DocumentsError::TooLarge {
                what: "document content (bytes)",
                size,
                limit,
            });
        }
        Ok(())
    }

    pub(super) async fn revision_bytes(
        &self,
        revision: &DocumentRevision,
    ) -> Result<Vec<u8>, DocumentsError> {
        Ok(self
            .blobs
            .read_all(&revision.content_sha256, self.config.max_upload_bytes)
            .await?)
    }

    pub(super) async fn parent_bytes(
        &self,
        revision: &DocumentRevision,
    ) -> Result<Vec<u8>, DocumentsError> {
        Ok(self
            .blobs
            .read_all_primary(&revision.content_sha256, self.config.max_upload_bytes)
            .await?)
    }

    pub(super) async fn plain_text_of(
        &self,
        format: DocumentFormat,
        bytes: Vec<u8>,
    ) -> Result<String, DocumentsError> {
        let codec = self.codec(format)?;
        blocking(move || Ok(codec.plain_text(&bytes)?)).await
    }

    pub async fn recipients(&self, document: &Document) -> Vec<UserId> {
        let mut users = BTreeSet::new();
        users.insert(document.owner_user_id.clone());
        let present = match self.repos.presence.users_present(&document.id).await {
            Ok(present) => present,
            Err(error) => {
                tracing::warn!(
                    document_id = %document.id,
                    error = %error,
                    "Present users could not be read; the event reaches the owner only"
                );
                Vec::new()
            },
        };
        for user in present {
            if users.contains(&user) {
                continue;
            }
            match self.access.level_of(document, &user).await {
                Ok(Some(_)) => {
                    users.insert(user);
                },
                Ok(None) => tracing::debug!(
                    document_id = %document.id,
                    user_id = %user,
                    "Present user no longer holds access; event withheld"
                ),
                Err(error) => tracing::warn!(
                    document_id = %document.id,
                    user_id = %user,
                    error = %error,
                    "Present user's access could not be read; event withheld"
                ),
            }
        }
        users.into_iter().collect()
    }

    pub(super) async fn folder_path(
        &self,
        folder: Option<&DocumentFolderId>,
    ) -> Result<Option<String>, DocumentsError> {
        match folder {
            Some(folder) => Ok(Some(self.repos.folders.path(folder).await?)),
            None => Ok(None),
        }
    }

    pub(super) async fn document_path(
        &self,
        document: &Document,
    ) -> Result<String, DocumentsError> {
        Ok(join_path(
            self.folder_path(document.folder_id.as_ref())
                .await?
                .as_deref(),
            &document.name,
        ))
    }
}

pub(super) fn join_path(folder: Option<&str>, name: &str) -> String {
    match folder {
        Some(folder) if !folder.is_empty() => format!("{folder}/{name}"),
        _ => name.to_owned(),
    }
}
