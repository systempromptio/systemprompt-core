//! Row positions of one apply batch.
//!
//! Every target in a batch names a row as the workbook stood before the
//! batch. An `insert_after` shifts the rows below it, so later targets are
//! moved by the inserts that came before them, and a second insert after
//! the same row lands after the first.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

#[derive(Debug, Default)]
pub(super) struct RowShifts {
    inserts: Vec<(String, u32)>,
    tails: HashMap<(String, u32), u32>,
}

impl RowShifts {
    pub(super) fn row(&self, sheet: &str, row: u32) -> u32 {
        self.inserts
            .iter()
            .filter(|(name, _)| name == sheet)
            .fold(row, |row, (_, at)| if row >= *at { row + 1 } else { row })
    }

    pub(super) fn anchor(&self, sheet: &str, row: u32) -> u32 {
        self.tails
            .get(&(sheet.to_owned(), row))
            .copied()
            .unwrap_or_else(|| self.row(sheet, row))
    }

    pub(super) fn inserted(&mut self, sheet: &str, anchor: u32, at: u32) {
        for ((name, _), tail) in &mut self.tails {
            if name == sheet && *tail >= at {
                *tail += 1;
            }
        }
        self.inserts.push((sheet.to_owned(), at));
        self.tails.insert((sheet.to_owned(), anchor), at);
    }
}
