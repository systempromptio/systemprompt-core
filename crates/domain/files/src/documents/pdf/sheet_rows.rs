//! The used range of a sheet as a grid of cell text, with a header row of
//! column labels.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_provider_contracts::documents::ProjectionNode;

use super::blocks::{rows_in, text_of};

fn column_index(id: &str) -> Option<usize> {
    let reference = id.rsplit('/').next()?;
    let letters: String = reference
        .chars()
        .take_while(char::is_ascii_uppercase)
        .collect();
    if letters.is_empty()
        || !reference[letters.len()..]
            .chars()
            .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let number = letters.chars().try_fold(0u32, |acc, c| {
        acc.checked_mul(26)?
            .checked_add(u32::from(c) - u32::from('A') + 1)
    })?;
    usize::try_from(number - 1).ok()
}

fn column_label(index: usize) -> String {
    let mut n = index + 1;
    let mut label = String::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        label.insert(0, char::from(b'A' + u8::try_from(rem).unwrap_or(0)));
        n = (n - 1) / 26;
    }
    label
}

pub(super) fn sheet_rows(sheet: &ProjectionNode) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    rows_in(sheet, &mut rows);
    let placed: Vec<(String, BTreeMap<usize, String>, String)> = rows
        .iter()
        .map(|row| {
            let number = row
                .id
                .as_str()
                .rsplit("row[")
                .next()
                .map(|rest| rest.trim_end_matches(']').to_owned())
                .unwrap_or_default();
            let cells: BTreeMap<usize, String> = row
                .children
                .iter()
                .filter_map(|cell| Some((column_index(cell.id.as_str())?, text_of(cell))))
                .collect();
            (number, cells, text_of(row))
        })
        .collect();
    let first = placed
        .iter()
        .filter_map(|(_, cells, _)| cells.keys().next().copied())
        .min()
        .unwrap_or(0);
    let last = placed
        .iter()
        .filter_map(|(_, cells, _)| cells.keys().next_back().copied())
        .max()
        .unwrap_or(0);
    let mut grid = Vec::with_capacity(placed.len() + 1);
    let mut header = vec![String::new()];
    header.extend((first..=last).map(column_label));
    grid.push(header);
    for (number, cells, fallback) in placed {
        let mut line = vec![number];
        if cells.is_empty() {
            line.push(fallback);
            line.resize(last - first + 2, String::new());
        } else {
            line.extend((first..=last).map(|col| cells.get(&col).cloned().unwrap_or_default()));
        }
        grid.push(line);
    }
    grid
}
