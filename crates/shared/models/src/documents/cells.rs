//! A validated A1 cell range on one sheet, `B2` or `B2:D9`, as a
//! comment anchored to a spreadsheet names it.
//!
//! The range is parsed once: columns run `A` to `XFD` (16 384), rows 1 to
//! 1 048 576, and the start is the top-left cell, never right of or below
//! the end. Letters are read case-insensitively and written upper-case, and
//! a range whose start is its end is written as that one cell.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub const MAX_CELL_COLUMN: u32 = 16_384;
pub const MAX_CELL_ROW: u32 = 1_048_576;

/// Why a text is not a [`CellRange`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CellRangeError {
    #[error("a cell range is an A1 reference such as `B2` or `B2:D9`")]
    Malformed,
    #[error("a cell range column is past XFD")]
    ColumnOutOfRange,
    #[error("a cell range row is past 1048576")]
    RowOutOfRange,
    #[error("a cell range starts at its top-left cell and ends at its bottom-right one")]
    Reversed,
}

/// A rectangle of cells, `(column, row)` 1-based, start top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CellRange {
    start: (u32, u32),
    end: (u32, u32),
}

impl CellRange {
    pub const fn new(start: (u32, u32), end: (u32, u32)) -> Result<Self, CellRangeError> {
        if let Err(error) = check_cell(start) {
            return Err(error);
        }
        if let Err(error) = check_cell(end) {
            return Err(error);
        }
        if start.0 > end.0 || start.1 > end.1 {
            return Err(CellRangeError::Reversed);
        }
        Ok(Self { start, end })
    }

    #[must_use]
    pub const fn start(&self) -> (u32, u32) {
        self.start
    }

    #[must_use]
    pub const fn end(&self) -> (u32, u32) {
        self.end
    }

    #[must_use]
    pub const fn contains(&self, col: u32, row: u32) -> bool {
        col >= self.start.0 && col <= self.end.0 && row >= self.start.1 && row <= self.end.1
    }
}

const fn check_cell((col, row): (u32, u32)) -> Result<(), CellRangeError> {
    if col == 0 || row == 0 {
        return Err(CellRangeError::Malformed);
    }
    if col > MAX_CELL_COLUMN {
        return Err(CellRangeError::ColumnOutOfRange);
    }
    if row > MAX_CELL_ROW {
        return Err(CellRangeError::RowOutOfRange);
    }
    Ok(())
}

fn column_letters(mut col: u32) -> String {
    let mut letters = Vec::new();
    while col > 0 {
        let rem = (col - 1) % 26;
        letters.push(char::from(b'A' + u8::try_from(rem).unwrap_or(0)));
        col = (col - 1) / 26;
    }
    letters.iter().rev().collect()
}

fn parse_cell(text: &str) -> Result<(u32, u32), CellRangeError> {
    let split = text
        .find(|c: char| !c.is_ascii_alphabetic())
        .ok_or(CellRangeError::Malformed)?;
    let (letters, digits) = text.split_at(split);
    if letters.is_empty()
        || digits.is_empty()
        || digits.starts_with('0')
        || !digits.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(CellRangeError::Malformed);
    }
    if letters.len() > 3 {
        return Err(CellRangeError::ColumnOutOfRange);
    }
    if digits.len() > 7 {
        return Err(CellRangeError::RowOutOfRange);
    }
    let col = letters.bytes().fold(0u32, |acc, b| {
        acc * 26 + u32::from(b.to_ascii_uppercase() - b'A' + 1)
    });
    let row = digits
        .parse::<u32>()
        .map_err(|_overflow| CellRangeError::RowOutOfRange)?;
    check_cell((col, row))?;
    Ok((col, row))
}

impl FromStr for CellRange {
    type Err = CellRangeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (start, end) = if let Some((start, end)) = text.split_once(':') {
            (parse_cell(start)?, parse_cell(end)?)
        } else {
            let only = parse_cell(text)?;
            (only, only)
        };
        Self::new(start, end)
    }
}

impl fmt::Display for CellRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", column_letters(self.start.0), self.start.1)?;
        if self.end != self.start {
            write!(f, ":{}{}", column_letters(self.end.0), self.end.1)?;
        }
        Ok(())
    }
}

impl TryFrom<String> for CellRange {
    type Error = CellRangeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<CellRange> for String {
    fn from(range: CellRange) -> Self {
        range.to_string()
    }
}

impl schemars::JsonSchema for CellRange {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "CellRange".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": "^[A-Za-z]{1,3}[1-9][0-9]{0,6}(:[A-Za-z]{1,3}[1-9][0-9]{0,6})?$"
        })
    }
}
