//! The `.xlsx` document codec.
//!
//! A workbook projects to one `Sheet` node per worksheet
//! (`sheet[<name>]`, `]` escaped as `\]`), one `Row` node per non-empty row
//! (`sheet[<name>]/row[<n>]`, text = tab-joined display values) and one
//! `Cell` node per non-empty cell (`sheet[<name>]/<A1>`). A formula cell
//! displays its cached value, or `=<formula>` when none is cached. Cell
//! children stop after 2,000 cells per sheet and the sheet is marked
//! `truncated`; [`XlsxCodec::project_range`] reads the rest.
//!
//! Edits are applied with `umya-spreadsheet` onto the parsed workbook, so
//! styles, merged cells, formulas and charts outside the edited cells
//! survive. Clearing a row empties its cells and never removes the row, so
//! formula references stay stable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod apply;
mod blank;
mod cells;
mod insert;
mod projection;
mod rebase;
mod workbook;

use std::sync::Arc;

use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, Projection, ProjectionNode, TemplateParams,
};

pub use blank::blank_xlsx;

/// The `.xlsx` [`DocumentCodec`], registered at compile time.
#[derive(Debug, Clone, Copy, Default)]
pub struct XlsxCodec;

impl XlsxCodec {
    pub fn project_range(
        bytes: &[u8],
        sheet: &str,
        a1_range: &str,
    ) -> Result<Vec<ProjectionNode>, CodecError> {
        projection::project_range(bytes, sheet, a1_range)
    }
}

impl DocumentCodec for XlsxCodec {
    fn insert_kinds(&self, _format: DocumentFormat) -> &'static [&'static str] {
        &["row", "pending"]
    }

    fn formats(&self) -> &'static [DocumentFormat] {
        &[DocumentFormat::Xlsx]
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        projection::project(bytes)
    }

    fn apply(
        &self,
        bytes: &[u8],
        _base: &Projection,
        ops: &[EditOp],
    ) -> Result<Vec<u8>, CodecError> {
        apply::apply(bytes, ops)
    }

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError> {
        projection::preview_html(bytes)
    }

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError> {
        projection::plain_text(bytes)
    }

    fn instantiate(&self, template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        blank::instantiate(template, params)
    }
}

fn xlsx_codec() -> Arc<dyn DocumentCodec> {
    Arc::new(XlsxCodec)
}

systemprompt_provider_contracts::register_document_codec!(xlsx_codec);
