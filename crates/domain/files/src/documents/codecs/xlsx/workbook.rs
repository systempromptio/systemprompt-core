//! Workbook I/O, A1 addressing and node-id helpers shared by the xlsx
//! projection and apply paths.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::io::Cursor;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::CodecError;
use umya_spreadsheet::{Cell, Workbook, Worksheet, XlsxError};

pub(super) type RowMap<'a> = BTreeMap<u32, BTreeMap<u32, &'a Cell>>;

pub(super) fn load(bytes: &[u8]) -> Result<Workbook, CodecError> {
    umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true).map_err(|source| {
        CodecError::Parse {
            part: "workbook".to_owned(),
            source: Box::new(source),
        }
    })
}

pub(super) fn save(workbook: &Workbook) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(workbook, &mut out).map_err(|source| {
        CodecError::Parse {
            part: "workbook".to_owned(),
            source: Box::new(source),
        }
    })?;
    Ok(out)
}

pub(super) fn sheet_error(node: DocumentNodeId) -> impl FnOnce(XlsxError) -> CodecError {
    move |source| match source {
        XlsxError::NotFound() => CodecError::UnknownNode { node },
        other => CodecError::Parse {
            part: "workbook".to_owned(),
            source: Box::new(other),
        },
    }
}

pub(super) fn display(cell: &Cell) -> String {
    let value = cell.value().into_owned();
    if cell.is_formula() && value.is_empty() {
        format!("={}", cell.formula())
    } else {
        value
    }
}

pub(super) fn cell_type(cell: &Cell) -> &'static str {
    match cell.data_type() {
        "n" => "n",
        "b" => "b",
        "e" => "e",
        "d" => "d",
        _ => "s",
    }
}

pub(super) fn rows(sheet: &Worksheet) -> RowMap<'_> {
    let mut rows: RowMap<'_> = BTreeMap::new();
    for cell in sheet.cells() {
        if display(cell).is_empty() {
            continue;
        }
        let coordinate = cell.coordinate();
        rows.entry(coordinate.row_num())
            .or_default()
            .insert(coordinate.col_num(), cell);
    }
    rows
}

pub(super) fn row_text(cells: &BTreeMap<u32, &Cell>) -> String {
    let last = cells.keys().next_back().copied().unwrap_or(0);
    (1..=last)
        .map(|col| {
            cells
                .get(&col)
                .map(|cell| display(cell))
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("\t")
}

pub(super) fn sheet_row_text(sheet: &Worksheet, row: u32) -> String {
    let cells: BTreeMap<u32, &Cell> = sheet
        .collection_by_row(row)
        .into_iter()
        .filter(|cell| !display(cell).is_empty())
        .map(|cell| (cell.coordinate().col_num(), cell))
        .collect();
    row_text(&cells)
}

pub(super) fn cell_display(sheet: &Worksheet, col: u32, row: u32) -> String {
    sheet.cell((col, row)).map(display).unwrap_or_default()
}

pub(super) fn column_letters(mut col: u32) -> String {
    let mut letters = Vec::new();
    while col > 0 {
        let rem = (col - 1) % 26;
        letters.push(char::from(b'A' + u8::try_from(rem).unwrap_or(0)));
        col = (col - 1) / 26;
    }
    letters.iter().rev().collect()
}

pub(super) fn a1(col: u32, row: u32) -> String {
    format!("{}{row}", column_letters(col))
}

pub(super) fn parse_a1(text: &str) -> Option<(u32, u32)> {
    let text = text.trim().replace('$', "");
    let split = text.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = text.split_at(split);
    if letters.is_empty() || letters.len() > 3 || !letters.chars().all(|c| c.is_ascii_alphabetic())
    {
        return None;
    }
    let col = letters.chars().fold(0u32, |acc, c| {
        acc * 26 + u32::from(c.to_ascii_uppercase() as u8 - b'A' + 1)
    });
    let row = digits.parse::<u32>().ok().filter(|row| *row > 0)?;
    Some((col, row))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CellRange {
    pub start: (u32, u32),
    pub end: (u32, u32),
}

impl CellRange {
    pub(super) fn parse(text: &str) -> Option<Self> {
        let (start, end) = if let Some((a, b)) = text.split_once(':') {
            (parse_a1(a)?, parse_a1(b)?)
        } else {
            let only = parse_a1(text)?;
            (only, only)
        };
        Some(Self {
            start: (start.0.min(end.0), start.1.min(end.1)),
            end: (start.0.max(end.0), start.1.max(end.1)),
        })
    }

    pub(super) const fn width(&self) -> u32 {
        self.end.0 - self.start.0 + 1
    }

    pub(super) const fn height(&self) -> u32 {
        self.end.1 - self.start.1 + 1
    }

    pub(super) fn contains(&self, col: u32, row: u32) -> bool {
        (self.start.0..=self.end.0).contains(&col) && (self.start.1..=self.end.1).contains(&row)
    }

    pub(super) fn text(&self, sheet: &Worksheet) -> String {
        (self.start.1..=self.end.1)
            .map(|row| {
                (self.start.0..=self.end.0)
                    .map(|col| cell_display(sheet, col, row))
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub(super) fn sheet_id(name: &str) -> String {
    format!("sheet[{}]", name.replace(']', "\\]"))
}

pub(super) fn node_id(text: String) -> DocumentNodeId {
    DocumentNodeId::new(text)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Target {
    Sheet,
    Row(u32),
    Cell(u32, u32),
}

pub(super) fn parse_node(id: &DocumentNodeId) -> Option<(String, Target)> {
    let rest = id.as_str().strip_prefix("sheet[")?;
    let mut name = String::new();
    let mut chars = rest.char_indices();
    let mut tail_start = None;
    while let Some((index, c)) = chars.next() {
        match c {
            '\\' if rest[index + 1..].starts_with(']') => {
                name.push(']');
                chars.next();
            },
            ']' => {
                tail_start = Some(index + 1);
                break;
            },
            other => name.push(other),
        }
    }
    let tail = &rest[tail_start?..];
    if tail.is_empty() {
        return Some((name, Target::Sheet));
    }
    let tail = tail.strip_prefix('/')?;
    if let Some(row) = tail.strip_prefix("row[").and_then(|t| t.strip_suffix(']')) {
        return Some((name, Target::Row(row.parse().ok().filter(|r| *r > 0)?)));
    }
    let (col, row) = parse_a1(tail)?;
    Some((name, Target::Cell(col, row)))
}
