//! Ruled grids. A table wider than the content area is split into column
//! groups drawn one under another; a row taller than a page is cut to the
//! lines that fit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(
    clippy::suboptimal_flops,
    reason = "grid geometry is a handful of point additions; mul_add obscures them"
)]

use super::fonts::Font;
use super::layout::{CONTENT_WIDTH, Layout, MARGIN, PageCapExceeded, Rule, Text, lines_f32, whole};
use super::wrap::wrap;

const CELL_SIZE: f32 = 9.5;
const CELL_LEADING: f32 = 12.0;
const CELL_PAD: f32 = 3.0;
const MIN_COLUMN: f32 = 48.0;
const GROUP_GAP: f32 = 8.0;

pub(super) fn place(layout: &mut Layout, rows: &[Vec<String>]) -> Result<(), PageCapExceeded> {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
        return Ok(());
    }
    let per_group = whole(CONTENT_WIDTH / MIN_COLUMN).max(1);
    let mut start = 0;
    while start < columns {
        let end = (start + per_group).min(columns);
        let width = CONTENT_WIDTH / lines_f32(end - start);
        for row in rows {
            let cells: Vec<&str> = (start..end)
                .map(|index| row.get(index).map_or("", String::as_str))
                .collect();
            place_row(layout, &cells, width)?;
        }
        let y = layout.y();
        layout.page_mut().rules.push(Rule {
            x1: MARGIN,
            y1: y,
            x2: MARGIN + width * lines_f32(end - start),
            y2: y,
        });
        layout.advance(GROUP_GAP);
        start = end;
    }
    Ok(())
}

fn place_row(layout: &mut Layout, cells: &[&str], width: f32) -> Result<(), PageCapExceeded> {
    let wrapped: Vec<Vec<String>> = cells
        .iter()
        .map(|cell| wrap(cell, Font::Sans, CELL_SIZE, width - 2.0 * CELL_PAD))
        .collect();
    let tallest = wrapped.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let full_height = CELL_LEADING * lines_f32(tallest) + 2.0 * CELL_PAD;
    layout.ensure(full_height)?;
    let fitting = whole((layout.available() - 2.0 * CELL_PAD) / CELL_LEADING);
    let lines = tallest.min(fitting.max(1));
    let height = CELL_LEADING * lines_f32(lines) + 2.0 * CELL_PAD;
    let top = layout.y();
    let bottom = top - height;
    let page = layout.page_mut();
    page.rules.push(Rule {
        x1: MARGIN,
        y1: top,
        x2: MARGIN + width * lines_f32(cells.len()),
        y2: top,
    });
    for (index, cell) in wrapped.iter().enumerate() {
        let x = MARGIN + width * lines_f32(index);
        page.rules.push(Rule {
            x1: x,
            y1: top,
            x2: x,
            y2: bottom,
        });
        for (line_index, line) in cell.iter().take(lines).enumerate() {
            page.texts.push(Text {
                x: x + CELL_PAD,
                y: top - CELL_PAD - CELL_SIZE - CELL_LEADING * lines_f32(line_index),
                font: Font::Sans,
                size: CELL_SIZE,
                content: line.clone(),
            });
        }
    }
    let right = MARGIN + width * lines_f32(cells.len());
    page.rules.push(Rule {
        x1: right,
        y1: top,
        x2: right,
        y2: bottom,
    });
    layout.advance(height);
    Ok(())
}
