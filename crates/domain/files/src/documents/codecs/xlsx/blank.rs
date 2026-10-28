//! The blank workbook new spreadsheets start from, and `{{key}}`
//! placeholder instantiation of workbook templates.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::{CodecError, TemplateParams};

use super::workbook::{load, save};

pub fn blank_xlsx() -> Result<Vec<u8>, CodecError> {
    save(&umya_spreadsheet::new_file())
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
