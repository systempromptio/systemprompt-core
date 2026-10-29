//! The `OpenDocument` codecs: `.odt` text, `.ods` spreadsheets and `.odp`
//! presentations.
//!
//! Every format keeps its content in `content.xml`; the codecs read and
//! write only that part. `styles.xml` is read only to resolve a master page
//! an inserted slide names. An edit rewrites the package so that every other
//! entry keeps its name, position and compressed bytes: `mimetype` stays the
//! first, stored entry and `META-INF/manifest.xml` is untouched. Inside
//! `content.xml` an edit splices only the byte ranges of the elements it
//! touches.
//!
//! - [`OdtCodec`] projects the body's `text:h` (heading at its
//!   `text:outline-level`), `text:p` and list paragraphs as `body/p[N]`, or
//!   `xid:<id>` when the paragraph carries `xml:id`/`text:id`, and tables as
//!   `body/table[T]/tr[R]/tc[C]/p[P]`.
//! - [`OdsCodec`] projects each `table:table` as `sheet[<name>]`, its non-empty
//!   rows as `sheet[<name>]/row[R]` and cells as `sheet[<name>]/<A1>`,
//!   expanding repeated rows and columns virtually and stopping at 2,000 cells
//!   a sheet (the sheet then carries `truncated=true`). `set_cells` and
//!   `replace_text` on a cell write `office:value-type`, `office:value` and the
//!   displayed `text:p`; text beginning with `=` is written as
//!   `table:formula="of:=…"` in `OpenFormula` syntax with no cached value, so
//!   the value appears once the document is recalculated.
//! - [`OdpCodec`] projects each `draw:page` as `slide[N]` (1-based), its shapes
//!   as `slide[N]/shape[S]` with `presentation:class` as the `placeholder`
//!   attribute, and their paragraphs as `slide[N]/shape[S]/p[P]`.
//!   `insert_slide` writes a page with the reference slide's master and layout,
//!   a title frame and an outline frame copied from the reference slide's
//!   frames of the same class; `layout` names a master page from `styles.xml`.
//!
//! `replace_text` rewrites a paragraph's content as one span carrying the
//! first span's `text:style-name`, so inline formatting, anchored frames,
//! notes and bookmarks inside that paragraph are dropped. Text in headers,
//! footers, notes and speaker notes is not projected.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod a1;
mod apply;
mod blank;
mod deck;
mod deck_insert;
mod fragment;
mod instantiate;
mod markdown;
mod odp;
mod ods;
mod odt;
mod package;
mod projection;
mod rich;
mod sheet;
mod sheet_edit;
mod sheet_view;
mod text;

use std::sync::Arc;

use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, Projection, ProjectionNode, content_hash,
};
use systemprompt_provider_contracts::register_document_codec;

pub use blank::{blank_odp, blank_ods, blank_odt};
pub use rich::{import_rich as import_odt_rich, materialise_rich as materialise_odt_rich};

use crate::documents::codecs::xml_tree::parse_xml;
use crate::documents::codecs::zip_package::Package;
use package::{CONTENT, Content, Splice, apply_splices, malformed};

/// The `.odt` [`DocumentCodec`]; registered at priority 100.
#[derive(Debug, Clone, Copy, Default)]
pub struct OdtCodec;

/// The `.ods` [`DocumentCodec`]; registered at priority 100.
#[derive(Debug, Clone, Copy, Default)]
pub struct OdsCodec;

/// The `.odp` [`DocumentCodec`]; registered at priority 100.
#[derive(Debug, Clone, Copy, Default)]
pub struct OdpCodec;

fn check_base(bytes: &[u8], base: &Projection) -> Result<(), CodecError> {
    if base.content_sha256 == content_hash(bytes) {
        Ok(())
    } else {
        Err(malformed(
            "the base projection was not read from these document bytes",
        ))
    }
}

fn read(bytes: &[u8]) -> Result<(Package<'_>, Content), CodecError> {
    let mut package = Package::open(bytes)?;
    let content = Content::read(&mut package)?;
    Ok((package, content))
}

fn finish(
    mut package: Package<'_>,
    xml: &str,
    splices: Vec<Splice>,
) -> Result<Vec<u8>, CodecError> {
    let edited = apply_splices(xml, splices)?;
    parse_xml(CONTENT, &edited)?;
    package.write(CONTENT, edited);
    package.finish()
}

fn projection(format: DocumentFormat, bytes: &[u8], root: Vec<ProjectionNode>) -> Projection {
    Projection {
        format,
        root,
        content_sha256: content_hash(bytes),
    }
}

fn odt_codec() -> Arc<dyn DocumentCodec> {
    Arc::new(OdtCodec)
}

fn ods_codec() -> Arc<dyn DocumentCodec> {
    Arc::new(OdsCodec)
}

fn odp_codec() -> Arc<dyn DocumentCodec> {
    Arc::new(OdpCodec)
}

register_document_codec!(odt_codec);
register_document_codec!(ods_codec);
register_document_codec!(odp_codec);
