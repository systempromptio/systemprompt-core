//! A `.docx` package as the `RichText` model, and the model written back.
//!
//! [`import_rich`] reads the body block by block (see the `import` and
//! `read` modules) and the document settings: styles, theme, numbering and the
//! final section with its headers and footers. [`materialise_rich_with`]
//! splices the model back over the base: a block equal to the base's
//! segment keeps its bytes, and only new or edited blocks are written
//! (the `write` modules). It then writes what those blocks need (picture parts,
//! numbering, header and footer parts, settings, relationships and content
//! types) and the settings the model changed (final section, styles,
//! theme). Every part nothing touched keeps its compressed bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_provider_contracts::documents::CodecError;

use super::import::{final_sect, insert_point, read_ctx, segments, settings};
use super::links::Links;
use super::numbering::Numbering;
use super::package::PartEdits;
use super::parts::{DOCUMENT, read_document};
use super::sect_splice::splice_sect_pr;
use super::sections::{finish_settings, sect_pr};
use super::write::{WriteCtx, write_block};
use super::write_drawing::max_drawing_id;
use super::write_styles::{finish_styles, finish_theme};
use crate::documents::model::layout::Section;
use crate::documents::model::office::SegmentedBody;
use crate::documents::model::{BlockKind, DocumentSettings, Media, RichText, office};

pub fn import_rich(bytes: &[u8]) -> Result<RichText, CodecError> {
    let src = read_document(bytes)?;
    let ctx = read_ctx(bytes)?;
    let mut model = office::import(&segments(&src, &ctx)?);
    let settings = settings(bytes, &src, &ctx)?;
    if !settings.is_default() {
        model.document = Some(Box::new(settings));
        office::number_running(&mut model);
    }
    Ok(model)
}

pub fn materialise_rich(base: &[u8], model: &RichText) -> Result<Vec<u8>, CodecError> {
    materialise_rich_with(base, model, &Media::new())
}

pub fn materialise_rich_with(
    base: &[u8],
    model: &RichText,
    media: &Media,
) -> Result<Vec<u8>, CodecError> {
    let src = read_document(base)?;
    let read = read_ctx(base)?;
    let body = segments(&src, &read)?;
    let base_settings = settings(base, &src, &read)?;
    let mut ctx = WriteCtx {
        base,
        links: Links::load(base, &src)?,
        numbering: Numbering::load(base)?,
        settings: model.settings(),
        media,
        edits: PartEdits::default(),
        error: None,
        drawing_id: max_drawing_id(&src),
        part_seq: 0,
        in_part: false,
        even_odd: false,
        update_fields: false,
        base_breaks: base_breaks(&src, &body),
        saved_uploads: HashMap::new(),
    };
    let mut xml = office::splice(&src, &body, model, |block| write_block(block, &mut ctx));
    xml = with_final_section(xml, &base_settings, &mut ctx);
    finish_styles(&mut ctx)?;
    finish_theme(&mut ctx, &read)?;
    finish_settings(&mut ctx)?;
    if let Some(error) = ctx.error.take() {
        return Err(error);
    }
    let WriteCtx {
        mut links,
        numbering,
        mut edits,
        ..
    } = ctx;
    numbering.finish(&mut edits, &mut links)?;
    links.finish_into(&mut edits)?;
    if xml != src {
        edits.put(DOCUMENT, xml.into_bytes());
    }
    edits.write(base)
}

fn with_final_section(xml: String, base: &DocumentSettings, ctx: &mut WriteCtx<'_>) -> String {
    let Some(section) = ctx.settings.section.clone() else {
        return xml;
    };
    if base
        .section
        .as_ref()
        .is_some_and(|held| same_section(held, &section))
    {
        return xml;
    }
    if let Some(range) = final_sect(&xml) {
        let written = match &base.section {
            Some(held) => splice_sect_pr(&xml[range.clone()], held, &section, ctx),
            None => sect_pr(&section, ctx),
        };
        format!("{}{written}{}", &xml[..range.start], &xml[range.end..])
    } else {
        let written = sect_pr(&section, ctx);
        let at = insert_point(&xml);
        format!("{}{written}{}", &xml[..at], &xml[at..])
    }
}

fn same_section(a: &Section, b: &Section) -> bool {
    let kind = |section: &Section| BlockKind::SectionBreak {
        section: Box::new(section.clone()),
    };
    office::same_kind(&kind(a), &kind(b))
}

fn base_breaks(src: &str, body: &SegmentedBody) -> HashMap<String, (String, Section)> {
    let base = office::import(body);
    body.segments
        .iter()
        .zip(&base.blocks)
        .filter_map(|(segment, block)| {
            let BlockKind::SectionBreak { section } = &block.kind else {
                return None;
            };
            let xml = &src[segment.range.clone()];
            let start = xml.find("<w:sectPr")?;
            let end = xml.rfind("</w:sectPr>")? + "</w:sectPr>".len();
            Some((
                block.id.clone(),
                (xml[start..end].to_owned(), (**section).clone()),
            ))
        })
        .collect()
}
