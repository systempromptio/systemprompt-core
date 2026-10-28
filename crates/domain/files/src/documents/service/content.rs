//! A new document's content given as a `RichText` model rather than ops.
//!
//! The template is instantiated first, so its theme, styles, numbering and
//! page setup with headers and footers (its branding) are the document's
//! settings; the content's blocks replace the template's body, and any
//! settings the content names override or join the template's: styles and
//! numbering definitions by id, the theme, the final section and front
//! matter whole. The model is validated like a live update (no raw markup,
//! the block, mark and link allowlists, the limits) before it is written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ContentHash;
use systemprompt_models::documents::{DocumentFormat, EditOp};
use systemprompt_provider_contracts::documents::{CodecError, DocumentCodec, resolve_targets};

use crate::documents::error::DocumentsError;
use crate::documents::model::document::DocumentSettings;
use crate::documents::model::formats::{ModelError, import_rich_text, materialise_rich_text_with};
use crate::documents::model::ops::apply_rich_ops_with;
use crate::documents::model::{EditLevel, Limits, Media, RichText, office, validate};

pub(super) fn apply_content(
    format: DocumentFormat,
    bytes: &[u8],
    content: &RichText,
    media: &Media,
) -> Result<Vec<u8>, DocumentsError> {
    let template = import_rich_text(format, bytes).map_err(model_error)?;
    let mut model = RichText::of(content.blocks.clone());
    let mut settings = template.settings();
    if let Some(given) = content.document.as_deref() {
        merge(&mut settings, given);
    }
    if !settings.is_default() {
        model.document = Some(Box::new(settings));
    }
    office::renumber(&mut model);
    validate(&template, &model, EditLevel::Edit, &Limits::default()).map_err(|rejection| {
        DocumentsError::Codec(CodecError::Refused {
            detail: format!("the content model was refused: {}", rejection.reason()),
        })
    })?;
    materialise_rich_text_with(format, bytes, &model, media).map_err(model_error)
}

fn merge(settings: &mut DocumentSettings, given: &DocumentSettings) {
    for style in &given.styles {
        match settings.styles.iter_mut().find(|held| held.id == style.id) {
            Some(held) => *held = style.clone(),
            None => settings.styles.push(style.clone()),
        }
    }
    for def in &given.numbering {
        match settings.numbering.iter_mut().find(|held| held.id == def.id) {
            Some(held) => *held = def.clone(),
            None => settings.numbering.push(def.clone()),
        }
    }
    if given.theme.is_some() {
        settings.theme.clone_from(&given.theme);
    }
    if given.section.is_some() {
        settings.section.clone_from(&given.section);
    }
    if given.front_matter.is_some() {
        settings.front_matter.clone_from(&given.front_matter);
    }
}

fn model_error(error: ModelError) -> DocumentsError {
    match error {
        ModelError::Codec(error) => DocumentsError::from(error),
        other => DocumentsError::Codec(CodecError::Refused {
            detail: other.to_string(),
        }),
    }
}

pub(super) fn apply_rich_on_new(
    codec: &dyn DocumentCodec,
    format: DocumentFormat,
    bytes: &[u8],
    ops: Vec<EditOp>,
    media: &Media,
) -> Result<Vec<u8>, DocumentsError> {
    let projection = codec.project(bytes)?;
    let resolved = resolve_targets(&projection, ops)?;
    Ok(apply_rich_ops_with(
        format,
        bytes,
        &projection,
        &resolved.ops,
        media,
    )?)
}

impl super::DocumentService {
    pub async fn media_bytes(&self, blob: &ContentHash) -> Result<Vec<u8>, DocumentsError> {
        Ok(self
            .blobs
            .read_all_primary(blob, self.config.max_upload_bytes)
            .await?)
    }
}
