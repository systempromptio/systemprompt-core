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
//! Parts are addressed by the `p:`/`a:`/`r:` prefixes PowerPoint always
//! writes; text inside group shapes and `mc:AlternateContent` is not
//! projected.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod apply;
mod blank;
mod layout;
mod nodes;
mod package;
mod projection;
mod slides;
mod xml;

use std::sync::Arc;

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, Projection, TemplateParams, content_hash,
};
use systemprompt_provider_contracts::register_document_codec;

pub use blank::blank_pptx;

use package::Package;
use projection::{Content, Slide};
use xml::{Element, parse_xml};

/// The `.pptx` [`DocumentCodec`]; registered at priority 100.
#[derive(Debug, Clone, Copy, Default)]
pub struct PptxCodec;

impl DocumentCodec for PptxCodec {
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
        apply::apply(bytes, ops)
    }

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(preview(&projection::load(&mut Package::open(bytes)?)?))
    }

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError> {
        Ok(plain_text(&projection::load(&mut Package::open(bytes)?)?))
    }

    fn instantiate(&self, template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        instantiate(template, params)
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

fn collect<'e>(element: &'e Element, name: &str, out: &mut Vec<&'e Element>) {
    for child in &element.children {
        if child.name == name {
            out.push(child);
        } else {
            collect(child, name, out);
        }
    }
}

fn substitute(text: &str, params: &TemplateParams, escaped: bool) -> String {
    params
        .values
        .iter()
        .fold(text.to_owned(), |text, (key, value)| {
            let value = if escaped {
                escape(value.as_str()).into_owned()
            } else {
                value.clone()
            };
            text.replace(&format!("{{{{{key}}}}}"), &value)
        })
}

fn instantiate_slide(slide: &Slide, params: &TemplateParams) -> Result<Option<String>, CodecError> {
    let root = parse_xml(&slide.part, &slide.xml)?;
    let mut paras = Vec::new();
    collect(&root, "a:p", &mut paras);
    let mut edits = Vec::new();
    for p in paras {
        let para = projection::paragraph(&slide.part, &slide.xml, p, false)?;
        let mut runs = Vec::new();
        collect(p, "a:t", &mut runs);
        let mut run_texts = Vec::new();
        for run in &runs {
            run_texts.push(run.text(&slide.part, &slide.xml)?);
        }
        let split = params.values.keys().any(|key| {
            let token = format!("{{{{{key}}}}}");
            let in_runs: usize = run_texts
                .iter()
                .map(|text| text.matches(&token).count())
                .sum();
            in_runs != para.text.matches(&token).count()
        });
        if split {
            let text = substitute(&para.text, params, false);
            edits.push((
                p.start,
                p.end,
                apply::paragraph_xml(&para, para.ppr.as_deref(), &text),
            ));
            continue;
        }
        for run in runs {
            let raw = &slide.xml[run.inner_start..run.inner_end];
            let replaced = substitute(raw, params, true);
            if replaced != raw {
                edits.push((run.inner_start, run.inner_end, replaced));
            }
        }
    }
    Ok((!edits.is_empty()).then(|| xml::splice(&slide.xml, edits)))
}

fn instantiate(template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
    let mut package = Package::open(template)?;
    for slide in projection::load(&mut package)? {
        if let Some(xml) = instantiate_slide(&slide, params)? {
            package.write(&slide.part, xml);
        }
    }
    package.finish()
}

fn pptx_codec() -> Arc<dyn DocumentCodec> {
    Arc::new(PptxCodec)
}

register_document_codec!(pptx_codec);
