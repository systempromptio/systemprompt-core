//! Markdown export of the head revision. A format with a rich text model is
//! degraded to what Markdown holds, and every other format is written from its
//! projection: a sheet as one table per sheet, a deck as a text outline.
//!
//! Images are embedded as `data:` URIs, so the file displays outside Drive at
//! the cost of a third more bytes than the picture. Once the embedded total
//! passes [`EMBED_LIMIT`], later images become a note in the text. Whatever
//! the file leaves out is named in [`ExportedMarkdown::left_out`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use base64::Engine as _;
use systemprompt_identifiers::{DocumentId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentFormat;

use super::DocumentService;
use super::support::blocking;
use crate::documents::error::DocumentsError;
use crate::documents::model::formats::import_rich_text;
use crate::documents::model::markdown::{degrade, materialise};
use crate::documents::model::{BlockKind, RichText};
use crate::documents::pdf::{markdown_file_name, outline_markdown};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedMarkdown {
    pub file_name: String,
    pub markdown: String,
    pub left_out: Vec<String>,
}

const EMBED_LIMIT: usize = 20 * 1024 * 1024;

fn image_refs(model: &RichText, into: &mut Vec<String>) {
    for block in &model.blocks {
        if let BlockKind::Image { blob, .. } = &block.kind {
            into.push(blob.clone());
        }
        for nested in block.kind.nested() {
            image_refs(nested, into);
        }
    }
}

fn is_portable(blob: &str) -> bool {
    let lower = blob.to_ascii_lowercase();
    ["http:", "https:", "data:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

fn embed(model: &mut RichText, uris: &BTreeMap<String, String>) {
    for block in &mut model.blocks {
        if let BlockKind::Image { blob, alt, .. } = &block.kind {
            if !is_portable(blob) {
                if let Some(uri) = uris.get(blob).cloned() {
                    if let BlockKind::Image { blob, .. } = &mut block.kind {
                        *blob = uri;
                    }
                } else {
                    block.text = if alt.is_empty() {
                        "[image not included]".to_owned()
                    } else {
                        format!("[image not included: {alt}]")
                    };
                    block.kind = BlockKind::Paragraph;
                }
            }
            continue;
        }
        for nested in block.kind.nested_mut() {
            embed(nested, uris);
        }
    }
}

/// `block <id>: <what>` lines folded to what a reader can act on, each once.
fn summarise(warnings: Vec<String>) -> Vec<String> {
    let mut seen = Vec::new();
    for warning in warnings {
        let what = warning
            .strip_prefix("block ")
            .and_then(|rest| rest.split_once(": "))
            .map_or(warning.as_str(), |(_, what)| what)
            .to_owned();
        if !seen.contains(&what) {
            seen.push(what);
        }
    }
    seen
}

fn outline_left_out(format: DocumentFormat) -> Vec<String> {
    let what = if matches!(format, DocumentFormat::Pptx | DocumentFormat::Odp) {
        "pictures, shape positions and text formatting"
    } else if matches!(format, DocumentFormat::Xlsx | DocumentFormat::Ods) {
        "cell formatting, merged cells and charts"
    } else {
        return Vec::new();
    };
    vec![what.to_owned()]
}

const fn has_rich_model(format: DocumentFormat) -> bool {
    matches!(
        format,
        DocumentFormat::Docx
            | DocumentFormat::Odt
            | DocumentFormat::Markdown
            | DocumentFormat::Html
    )
}

impl DocumentService {
    pub async fn export_markdown(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        trace: &TraceId,
    ) -> Result<ExportedMarkdown, DocumentsError> {
        let (document, _, bytes) = self.read(who, document, None, trace).await?;
        if document.format.is_file() {
            return Err(DocumentsError::UnsupportedFormat {
                format: document.format,
            });
        }
        let codec = self.codec(document.format)?;
        let format = document.format;
        let source = bytes.clone();
        let rich = blocking(move || {
            Ok::<_, DocumentsError>(if has_rich_model(format) {
                import_rich_text(format, &source).ok().map(|m| degrade(&m))
            } else {
                None
            })
        })
        .await?;
        let (markdown, left_out) = if let Some((model, warnings)) = rich {
            self.rich_markdown(format, &bytes, model, warnings).await?
        } else {
            let text = blocking(move || {
                Ok::<_, DocumentsError>(outline_markdown(&codec.project(&bytes)?))
            })
            .await?;
            (text, outline_left_out(format))
        };
        Ok(ExportedMarkdown {
            file_name: markdown_file_name(&document.name),
            markdown,
            left_out,
        })
    }

    async fn rich_markdown(
        &self,
        format: DocumentFormat,
        bytes: &[u8],
        mut model: RichText,
        warnings: Vec<String>,
    ) -> Result<(String, Vec<String>), DocumentsError> {
        let mut left_out = summarise(warnings);
        let mut refs = Vec::new();
        image_refs(&model, &mut refs);
        refs.retain(|blob| !is_portable(blob));
        refs.sort();
        refs.dedup();
        let mut uris = BTreeMap::new();
        let mut total = 0;
        let mut missing = 0;
        for reference in refs {
            let found = self.image_in_revision(format, bytes, &reference).await;
            let uri = found.ok().flatten().map(|blob| {
                let body = base64::engine::general_purpose::STANDARD.encode(&blob.bytes);
                format!("data:{};base64,{body}", blob.mime)
            });
            match uri {
                Some(uri) if total + uri.len() <= EMBED_LIMIT => {
                    total += uri.len();
                    uris.insert(reference, uri);
                },
                _ => missing += 1,
            }
        }
        if missing > 0 {
            left_out.push(format!("{missing} image(s) too large or unreadable"));
        }
        embed(&mut model, &uris);
        let text = blocking(move || Ok::<_, DocumentsError>(materialise(&model)?)).await?;
        Ok((text, left_out))
    }
}
