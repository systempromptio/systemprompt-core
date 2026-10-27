//! The `.ods` codec's `DocumentCodec` implementation and its batch of cell
//! writes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CellValue, CodecError, DocumentCodec, DocumentFormat, EditOp, Projection, TemplateParams,
};

use super::package::{malformed, verify};
use super::{OdsCodec, a1, check_base, finish, instantiate, projection, read, sheet, sheet_edit};

fn parse_cell_id(id: &str) -> Option<(String, u32, u32)> {
    let (name, cell) = id.strip_prefix("sheet[")?.rsplit_once("]/")?;
    let (col, row) = a1::parse_a1(cell)?;
    Some((name.replace("\\]", "]"), col, row))
}

fn ods_apply(bytes: &[u8], ops: &[EditOp]) -> Result<Vec<u8>, CodecError> {
    let (package, content) = read(bytes)?;
    let sheets = sheet::read(&content)?;
    let find = |name: &str| {
        sheets
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| CodecError::UnknownNode {
                node: DocumentNodeId::new(a1::sheet_id(name)),
            })
    };
    let mut writes: BTreeMap<String, sheet_edit::RowWrites> = BTreeMap::new();
    for op in ops {
        match op {
            EditOp::SetCells {
                sheet,
                range,
                values,
                expected_hash,
            } => {
                let target = find(sheet)?;
                let (cols, rows) = a1::parse_range(range)
                    .ok_or(malformed("set_cells range is not an A1 reference"))?;
                let fits =
                    rows.len() == values.len() && values.iter().all(|row| row.len() == cols.len());
                if !fits {
                    return Err(malformed(
                        "set_cells values do not match the shape of the range",
                    ));
                }
                if let Some(expected) = expected_hash {
                    let node = format!("{}/{range}", a1::sheet_id(sheet));
                    verify(&node, &target.range_text(&cols, &rows), expected)?;
                }
                let sheet_writes = writes.entry(sheet.clone()).or_default();
                for (row, row_values) in rows.zip(values) {
                    for (col, value) in cols.clone().zip(row_values) {
                        sheet_writes
                            .entry(row)
                            .or_default()
                            .insert(col, value.clone());
                    }
                }
            },
            EditOp::ReplaceText {
                node,
                expected_hash,
                text,
            } => {
                let (name, col, row) = parse_cell_id(node.as_str())
                    .ok_or_else(|| CodecError::UnknownNode { node: node.clone() })?;
                let target = find(&name)?;
                verify(node.as_str(), &target.display(col, row), expected_hash)?;
                writes
                    .entry(name)
                    .or_default()
                    .entry(row)
                    .or_default()
                    .insert(col, CellValue::Text(text.clone()));
            },
            _ => {
                return Err(CodecError::UnsupportedOp {
                    op: op.name(),
                    format: DocumentFormat::Ods,
                });
            },
        }
    }
    let mut splices = Vec::new();
    for (name, sheet_writes) in &writes {
        splices.extend(sheet_edit::render(
            &content.xml,
            find(name)?,
            sheet_writes,
            0,
        ));
    }
    finish(package, &content.xml, splices)
}

impl DocumentCodec for OdsCodec {
    fn formats(&self) -> &'static [DocumentFormat] {
        &[DocumentFormat::Ods]
    }

    fn project(&self, bytes: &[u8]) -> Result<Projection, CodecError> {
        let (_, content) = read(bytes)?;
        let root = sheet::read(&content)?
            .iter()
            .enumerate()
            .map(|(index, sheet)| sheet.node(index))
            .collect();
        Ok(projection(DocumentFormat::Ods, bytes, root))
    }

    fn apply(
        &self,
        bytes: &[u8],
        base: &Projection,
        ops: &[EditOp],
    ) -> Result<Vec<u8>, CodecError> {
        check_base(bytes, base)?;
        if ops.is_empty() {
            return Ok(bytes.to_vec());
        }
        ods_apply(bytes, ops)
    }

    fn preview_html(&self, bytes: &[u8]) -> Result<String, CodecError> {
        let mut out = String::new();
        for sheet in sheet::read(&read(bytes)?.1)? {
            sheet.preview(&mut out);
        }
        Ok(out)
    }

    fn plain_text(&self, bytes: &[u8]) -> Result<String, CodecError> {
        let mut out = String::new();
        for sheet in sheet::read(&read(bytes)?.1)? {
            sheet.plain_text(&mut out);
        }
        Ok(out)
    }

    fn instantiate(&self, template: &[u8], params: &TemplateParams) -> Result<Vec<u8>, CodecError> {
        instantiate::instantiate(template, params)
    }
}
