//! Image admissions for documents whose revisions predate admission
//! records (the `document_media_backfill` queue).
//!
//! A queued document admits every stored image any of its revisions names,
//! each admitted by the earliest author of a revision naming it, and is then
//! dequeued in the same transaction. The queue is drained by the
//! `document_media_backfill` job, and a queued document is backfilled on its
//! first image read, image admission or copy, so it never serves or refuses
//! on admissions it is still owed. A revision whose bytes are gone or exceed
//! the read limit is logged and skipped; any other failure leaves the
//! document queued for the next attempt.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_traits::FileStorageError;

use super::DocumentService;
use super::markdown_export::has_rich_model;
use super::media_admission::store_refs;
use super::support::blocking;
use crate::documents::blob_store::BlobStoreError;
use crate::documents::error::DocumentsError;

impl DocumentService {
    pub(super) async fn ensure_media_backfilled(
        &self,
        document: &DocumentId,
    ) -> Result<(), DocumentsError> {
        if self.repos.media.backfill_queued(document).await? {
            self.backfill_media(document).await?;
        }
        Ok(())
    }

    pub async fn backfill_queued_media(&self, limit: i64) -> Result<u64, DocumentsError> {
        let mut done = 0;
        for document in self.repos.media.backfill_batch(limit).await? {
            self.backfill_media(&document).await?;
            done += 1;
        }
        Ok(done)
    }

    async fn backfill_media(&self, id: &DocumentId) -> Result<(), DocumentsError> {
        let mut admitted: BTreeMap<String, UserId> = BTreeMap::new();
        if let Some(document) = self.repos.documents.find(id).await?
            && has_rich_model(document.format)
        {
            let limit = self.config.max_upload_bytes;
            for (revision, author) in self.repos.media.revision_sources(id).await? {
                let bytes = match self.blobs.read_all_primary(&revision, limit).await {
                    Ok(bytes) => bytes,
                    Err(
                        error @ (BlobStoreError::NotFound { .. }
                        | BlobStoreError::TooLarge { .. }
                        | BlobStoreError::Storage(FileStorageError::NotFound(_))),
                    ) => {
                        tracing::warn!(
                            document = %id,
                            content_sha256 = %revision,
                            error = %error,
                            "A revision could not be read, so its images are not admitted"
                        );
                        continue;
                    },
                    Err(error) => return Err(error.into()),
                };
                let format = document.format;
                let refs = blocking(move || Ok(store_refs(format, &bytes, limit))).await?;
                for reference in refs {
                    admitted.entry(reference).or_insert_with(|| author.clone());
                }
            }
        }
        let (hashes, authors): (Vec<String>, Vec<String>) = admitted
            .into_iter()
            .map(|(hash, author)| (hash, author.as_str().to_owned()))
            .unzip();
        self.repos
            .media
            .complete_backfill(id, &hashes, &authors)
            .await?;
        Ok(())
    }
}
