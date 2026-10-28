//! A `docx` document exported with its open comment threads as native Word
//! comments.
//!
//! The head revision's bytes are written through the `docx` codec's comment
//! writer: every comment that is neither resolved nor deleted, anchored to a
//! paragraph, becomes a `w:comment` spanning that paragraph, under its
//! author's id and creation time. Viewing the document is enough; other
//! formats are refused as unsupported.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, DocumentFormat};
use systemprompt_provider_contracts::documents::CodecError;

use super::DocumentService;
use crate::documents::error::DocumentsError;
use crate::documents::models::DocumentComment;
use crate::documents::nodes::path_of;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedDocument {
    pub name: String,
    pub bytes: Vec<u8>,
}

impl DocumentService {
    pub async fn export_with_comments(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<ExportedDocument, DocumentsError> {
        let document = self
            .authorized_document(who, document, DocumentAction::View, trace)
            .await?;
        if document.format != DocumentFormat::Docx {
            return Err(DocumentsError::Codec(CodecError::UnsupportedOp {
                op: "export_with_comments",
                format: document.format,
            }));
        }
        let head = self.resolve_revision(&document, None).await?;
        let (projected, bytes) = self.projected_with_bytes(&document, &head).await?;
        let comments = self.repos.comments.list_for_document(&document.id).await?;
        let anchored: Vec<(String, DocumentComment)> = comments
            .into_iter()
            .filter(|comment| comment.resolved_at.is_none() && comment.deleted_at.is_none())
            .filter_map(|comment| {
                let node = projected.ids.find(&comment.anchor.node)?;
                Some((path_of(node).to_owned(), comment))
            })
            .collect();
        let bytes = super::support::blocking(move || native(&bytes, anchored)).await?;
        Ok(ExportedDocument {
            name: document.name,
            bytes,
        })
    }
}

#[cfg(feature = "office")]
fn native(
    bytes: &[u8],
    anchored: Vec<(String, DocumentComment)>,
) -> Result<Vec<u8>, DocumentsError> {
    use systemprompt_identifiers::DocumentNodeId;

    use crate::documents::codecs::docx::{NativeComment, with_comments};

    let comments: Vec<NativeComment> = anchored
        .into_iter()
        .map(|(path, comment)| NativeComment {
            node: DocumentNodeId::new(path),
            author: comment.author_user_id.as_str().to_owned(),
            date: comment
                .created_at
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            text: comment.body,
        })
        .collect();
    Ok(with_comments(bytes, &comments)?)
}

#[cfg(not(feature = "office"))]
fn native(
    _bytes: &[u8],
    _anchored: Vec<(String, DocumentComment)>,
) -> Result<Vec<u8>, DocumentsError> {
    Err(DocumentsError::Codec(CodecError::UnsupportedOp {
        op: "export_with_comments",
        format: DocumentFormat::Docx,
    }))
}
