//! The `.pptx` (`PresentationML`) document codec.
//!
//! [`PptxCodec`] reads the slide order from `p:sldIdLst` in
//! `ppt/presentation.xml` and projects each slide as `slide[N]` (1-based),
//! its shapes as `slide[N]/shape[S]` (0-based order in `p:spTree`), their
//! paragraphs as `slide[N]/shape[S]/p[P]`, tables as
//! `slide[N]/shape[S]/tbl/r[R]/c[C]`, pictures as image shapes carrying their
//! relationship id, and speaker notes read-only as `slide[N]/notes/p[P]`.
//! A paragraph counts as bulleted when it carries `a:buChar`/`a:buAutoNum`,
//! a level above zero, or sits in a body placeholder without `a:buNone`.
//!
//! An edit splices only the bytes of the paragraphs it touches inside one
//! `ppt/slides/slideN.xml`; inserting or removing a slide rewrites the
//! presentation, its relationships, `[Content_Types].xml` and the slide
//! count in `docProps/app.xml`. Every other zip entry keeps its name,
//! position and compressed bytes. A new slide is built from the placeholders
//! of its layout: the named layout, else the layout of the slide it follows.
//! Parts are addressed by the `p:`/`a:`/`r:` prefixes `PowerPoint` always
//! writes; text inside group shapes and `mc:AlternateContent` is not
//! projected.
//!
//! With `collab-loro`, [`deck_model`] reads a deck as the engine's live
//! deck model and [`lower_deck`] writes one back through the same splices.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "collab-loro")]
mod align;
mod apply;
mod blank;
#[cfg(feature = "collab-loro")]
mod deck;
mod fragment;
mod inherit;
mod layout;
#[cfg(feature = "collab-loro")]
mod lower;
mod nodes;
mod package;
mod paragraph;
#[cfg(feature = "collab-loro")]
mod picture;
mod projection;
mod rebase;
#[cfg(feature = "collab-loro")]
mod restyle;
#[cfg(feature = "collab-loro")]
mod runs;
mod slides;
#[cfg(feature = "collab-loro")]
mod structure;
mod template;
#[cfg(feature = "collab-loro")]
mod zorder;

use std::sync::Arc;

use quick_xml::escape::escape;
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, NodeFragment, NodeTarget, Projection,
    TemplateParams, content_hash, fragment_insert_op,
};
use systemprompt_provider_contracts::register_document_codec;

pub use blank::blank_pptx;
#[cfg(feature = "collab-loro")]
pub use deck::deck_model;
#[cfg(feature = "collab-loro")]
pub use lower::lower_deck;

use crate::documents::codecs::fragments::leaf_hashes;
use crate::documents::codecs::zip_package::Package;
use projection::{Content, Slide};

/// The `.pptx` [`DocumentCodec`]; registered at priority 100.
#[derive(Debug, Clone, Copy, Default)]
pub struct PptxCodec;

impl DocumentCodec for PptxCodec {
    fn insert_kinds(&self, _format: DocumentFormat) -> &'static [&'static str] {
        &["paragraph", "list_item", "pending"]
    }

    fn formats(&self) -> &'static [DocumentFormat] {
        &[DocumentFormat::Pptx]
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        let slides = projection::load(&mut Package::open(bytes)?)?;
        Ok(Projection {
            format: DocumentFormat::Pptx,
            root: nodes::nodes(&slides),
            content_sha256: content_hash(bytes),
        })
    }

    fn apply(
        &self,
        bytes: &[u8],
        base: &Projection,
        ops: &[EditOp],
    ) -> Result<Vec<u8>, CodecError> {
        if base.content_sha256 != content_hash(bytes) {
            return Err(CodecError::Malformed {
                detail: "the base projection was not read from these document bytes",
            });
        }
        rebase::apply(bytes, ops)
    }

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(preview(&projection::load(&mut Package::open(bytes)?)?))
    }

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(plain_text(&projection::load(&mut Package::open(bytes)?)?))
    }

    fn instantiate(&self, template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        template::instantiate(template, params)
    }

    fn style_hashes(&self, bytes: &[u8]) -> Result<Vec<(DocumentNodeId, String)>, CodecError> {
        Ok(leaf_hashes(
            &self.project(bytes)?,
            &fragment::styled(bytes)?,
        ))
    }

    fn copy_node(
        &self,
        bytes: &[u8],
        base: &Projection,
        path: &DocumentNodeId,
    ) -> Result<NodeFragment, CodecError> {
        fragment::copy(bytes, base, path)
    }

    fn insert_fragment(
        &self,
        bytes: &[u8],
        base: &Projection,
        after: &NodeTarget,
        fragment: &NodeFragment,
    ) -> Result<Vec<u8>, CodecError> {
        if fragment.native.is_none() {
            return self.apply(bytes, base, &[fragment_insert_op(after, fragment)]);
        }
        fragment::insert(bytes, base, after, fragment)
    }
}

fn plain_text(slides: &[Slide]) -> String {
    let mut out = String::new();
    for (index, slide) in slides.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&format!("# Slide {}", index + 1));
        out.push('\n');
        for shape in &slide.shapes {
            match &shape.content {
                Content::Text(paras) => paras.iter().for_each(|para| {
                    out.push_str(&para.text);
                    out.push('\n');
                }),
                Content::Table(rows) => rows.iter().for_each(|cells| {
                    out.push_str(&cells.join(" | "));
                    out.push('\n');
                }),
                Content::Image(_) | Content::Other => {},
            }
        }
        if !slide.notes.is_empty() {
            out.push_str("## Notes\n");
            slide.notes.iter().for_each(|line| {
                out.push_str(line);
                out.push('\n');
            });
        }
    }
    out
}

fn preview(slides: &[Slide]) -> String {
    let mut out = String::new();
    for (index, slide) in slides.iter().enumerate() {
        out.push_str(&format!("<section data-slide=\"{}\">", index + 1));
        if let Some(title) = slide.title() {
            out.push_str(&format!("<h2>{}</h2>", escape(title.as_str())));
        }
        for shape in slide.shapes.iter().filter(|shape| !shape.is_title()) {
            match &shape.content {
                Content::Text(paras) => {
                    let mut in_list = false;
                    for para in paras.iter().filter(|para| !para.text.trim().is_empty()) {
                        if para.bullet != in_list {
                            out.push_str(if para.bullet { "<ul>" } else { "</ul>" });
                            in_list = para.bullet;
                        }
                        let tag = if para.bullet { "li" } else { "p" };
                        out.push_str(&format!("<{tag}>{}</{tag}>", escape(para.text.as_str())));
                    }
                    if in_list {
                        out.push_str("</ul>");
                    }
                },
                Content::Table(rows) => {
                    out.push_str("<table>");
                    for cells in rows {
                        out.push_str("<tr>");
                        for cell in cells {
                            out.push_str(&format!("<td>{}</td>", escape(cell.as_str())));
                        }
                        out.push_str("</tr>");
                    }
                    out.push_str("</table>");
                },
                Content::Image(_) | Content::Other => {},
            }
        }
        out.push_str("</section>\n");
    }
    out
}

fn pptx_codec() -> Arc<dyn DocumentCodec> {
    Arc::new(PptxCodec)
}

register_document_codec!(pptx_codec);
