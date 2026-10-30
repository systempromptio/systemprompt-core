//! The plain workbook model the Loro sheet containers read into and write
//! from, with A1 addresses derived from id positions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use super::a1::{A1Range, a1};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetModel {
    pub sheets: Vec<Sheet>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sheet {
    pub id: String,
    pub name: String,
    pub rows: Vec<String>,
    pub cols: Vec<String>,
    pub cells: BTreeMap<String, SheetCell>,
    pub merges: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SheetCell {
    pub v: String,
    pub fmt: Option<String>,
    pub style: Option<String>,
}

#[must_use]
pub fn cell_key(row_id: &str, col_id: &str) -> String {
    format!("{row_id}:{col_id}")
}

/// A fresh row, column, sheet or cell id; random so concurrent replicas
/// never mint the same one.
#[must_use]
pub fn new_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_owned()
}

impl SheetModel {
    #[must_use]
    pub fn sheet(&self, name: &str) -> Option<&Sheet> {
        self.sheets.iter().find(|sheet| sheet.name == name)
    }

    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.sheets.iter().map(|sheet| sheet.name.clone()).collect()
    }
}

impl Sheet {
    /// A sheet named `name` with fresh ids, from 1-based `(col, row)`
    /// cells and A1 merge ranges.
    #[must_use]
    pub fn from_grid(
        name: &str,
        cells: impl IntoIterator<Item = (u32, u32, SheetCell)>,
        merges: &[String],
    ) -> Self {
        let cells: Vec<(u32, u32, SheetCell)> = cells.into_iter().collect();
        let ranges: Vec<A1Range> = merges.iter().filter_map(|m| A1Range::parse(m)).collect();
        let max_col = cells
            .iter()
            .map(|c| c.0)
            .chain(ranges.iter().map(|r| r.end.0))
            .max()
            .unwrap_or(0);
        let max_row = cells
            .iter()
            .map(|c| c.1)
            .chain(ranges.iter().map(|r| r.end.1))
            .max()
            .unwrap_or(0);
        let mut sheet = Self {
            id: new_id(),
            name: name.to_owned(),
            rows: (0..max_row).map(|_| new_id()).collect(),
            cols: (0..max_col).map(|_| new_id()).collect(),
            ..Self::default()
        };
        for (col, row, cell) in cells {
            if let Some((r, c)) = sheet.ids_at(col, row) {
                let key = cell_key(r, c);
                sheet.cells.insert(key, cell);
            }
        }
        sheet.merges = ranges
            .iter()
            .filter_map(|range| {
                let (r0, c0) = sheet.ids_at(range.start.0, range.start.1)?;
                let (r1, c1) = sheet.ids_at(range.end.0, range.end.1)?;
                Some(format!("{r0}:{c0}|{r1}:{c1}"))
            })
            .collect();
        sheet
    }

    /// The `(row id, col id)` at a 1-based `(col, row)`.
    #[must_use]
    pub fn ids_at(&self, col: u32, row: u32) -> Option<(&str, &str)> {
        let r = self.rows.get(usize::try_from(row).ok()?.checked_sub(1)?)?;
        let c = self.cols.get(usize::try_from(col).ok()?.checked_sub(1)?)?;
        Some((r.as_str(), c.as_str()))
    }

    /// The 1-based `(col, row)` a `"<rowId>:<colId>"` key sits at now.
    #[must_use]
    pub fn position(&self, key: &str) -> Option<(u32, u32)> {
        let (r, c) = key.split_once(':')?;
        let row = self.rows.iter().position(|id| id == r)?;
        let col = self.cols.iter().position(|id| id == c)?;
        Some((u32::try_from(col + 1).ok()?, u32::try_from(row + 1).ok()?))
    }

    #[must_use]
    pub fn a1_of(&self, key: &str) -> Option<String> {
        self.position(key).map(|(col, row)| a1(col, row))
    }

    /// A merge's A1 range, or `None` when a boundary id was deleted.
    #[must_use]
    pub fn merge_a1(&self, merge: &str) -> Option<String> {
        let (start, end) = merge.split_once('|')?;
        Some(format!("{}:{}", self.a1_of(start)?, self.a1_of(end)?))
    }

    /// Every cell with a live position, as 1-based `(col, row, cell)`,
    /// skipping cells whose row or column was deleted.
    #[must_use]
    pub fn grid(&self) -> Vec<(u32, u32, &SheetCell)> {
        let rows: BTreeMap<&str, u32> = (1u32..)
            .zip(&self.rows)
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        let cols: BTreeMap<&str, u32> = (1u32..)
            .zip(&self.cols)
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        self.cells
            .iter()
            .filter_map(|(key, cell)| {
                let (r, c) = key.split_once(':')?;
                Some((*cols.get(c)?, *rows.get(r)?, cell))
            })
            .collect()
    }
}
