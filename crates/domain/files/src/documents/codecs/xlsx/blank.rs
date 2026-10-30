//! The blank workbook new spreadsheets start from (written by `IronCalc`, so
//! the live sheet engine can read it back), and `{{key}}`
//! placeholder instantiation of workbook templates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::{CodecError, TemplateParams};

use std::io::Cursor;

use ironcalc::base::Model;
use ironcalc::export::save_xlsx_to_writer;

use super::workbook::{load, save};

pub fn blank_xlsx() -> Result<Vec<u8>, CodecError> {
    let model =
        Model::new_empty("workbook", "en", "UTC", "en").map_err(|detail| CodecError::Parse {
            part: "workbook".to_owned(),
            source: detail.into(),
        })?;
    let cursor = save_xlsx_to_writer(&model, Cursor::new(Vec::new())).map_err(|source| {
        CodecError::Parse {
            part: "workbook".to_owned(),
            source: Box::new(source),
        }
    })?;
    Ok(cursor.into_inner())
}

pub(super) fn instantiate(template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
    let mut workbook = load(template)?;
    for sheet in workbook.sheet_collection_mut() {
        for cell in sheet.cells_mut() {
            if cell.is_formula() || cell.data_type() != "s" {
                continue;
            }
            let original = cell.value().into_owned();
            if !original.contains("{{") {
                continue;
            }
            let filled = params
                .values
                .iter()
                .fold(original.clone(), |text, (key, value)| {
                    text.replace(&format!("{{{{{key}}}}}"), value)
                });
            if filled != original {
                cell.set_value_string(filled);
            }
        }
    }
    save(&workbook)
}
