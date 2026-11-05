//! Styled grids: a shaded, bold header that repeats on each new page, a light
//! rule between rows and the document's own cell shading. A table wider than
//! the content area is split into column groups drawn one under another; a
//! row taller than a page is cut to the lines that fit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(
    clippy::suboptimal_flops,
    reason = "grid geometry is a handful of point additions; mul_add obscures them"
)]

use super::blocks::Cell;
use super::layout::Layout;
use super::layout_model::{
    CONTENT_WIDTH, Fill, MARGIN, PageCapExceeded, Rect, Rule, lines_f32, whole,
};
use super::runs::Rgb;
use super::wrap::{Line, Measure, wrap};

const CELL_SIZE: f32 = 9.5;
const CELL_LEADING: f32 = 12.5;
const CELL_PAD: f32 = 4.0;
const MIN_COLUMN: f32 = 48.0;
const GROUP_GAP: f32 = 8.0;
const GRID: Rgb = [0xc8, 0xcd, 0xd4];
const HEADER_TINT: Rgb = [0xe9, 0xec, 0xf0];
const RULE_WIDTH: f32 = 0.5;

#[derive(Debug, Clone, Copy)]
struct Grid<'a> {
    width: f32,
    start: usize,
    end: usize,
    header: &'a [Vec<Cell>],
}

pub(super) fn place(
    layout: &mut Layout<'_>,
    rows: &[Vec<Cell>],
    header_rows: usize,
) -> Result<(), PageCapExceeded> {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    if columns == 0 {
        return Ok(());
    }
    let header_rows = header_rows.min(rows.len());
    let per_group = whole(CONTENT_WIDTH / MIN_COLUMN).max(1);
    let mut start = 0;
    while start < columns {
        let end = (start + per_group).min(columns);
        let grid = Grid {
            width: CONTENT_WIDTH / lines_f32(end - start),
            start,
            end,
            header: &rows[..header_rows],
        };
        for (index, row) in rows.iter().enumerate() {
            place_row(layout, row, grid, index < header_rows)?;
        }
        let y = layout.y();
        let right = MARGIN + grid.width * lines_f32(end - start);
        layout.page_mut().rules.push(rule((MARGIN, y), (right, y)));
        layout.advance(GROUP_GAP);
        start = end;
    }
    Ok(())
}

const fn rule(from: (f32, f32), to: (f32, f32)) -> Rule {
    Rule {
        x1: from.0,
        y1: from.1,
        x2: to.0,
        y2: to.1,
        color: GRID,
        width: RULE_WIDTH,
    }
}

const fn measure(width: f32, header: bool) -> Measure {
    Measure {
        size: CELL_SIZE,
        width: width - 2.0 * CELL_PAD,
        force_bold: header,
    }
}

fn wrapped(row: &[Cell], grid: Grid<'_>, header: bool) -> Vec<Vec<Line>> {
    (grid.start..grid.end)
        .map(|index| {
            let runs = row.get(index).map_or(&[][..], |cell| cell.runs.as_slice());
            wrap(runs, measure(grid.width, header))
        })
        .collect()
}

fn height_of(lines: &[Vec<Line>]) -> (usize, f32) {
    let tallest = lines.iter().map(Vec::len).max().unwrap_or(1).max(1);
    (tallest, CELL_LEADING * lines_f32(tallest) + 2.0 * CELL_PAD)
}

fn place_row(
    layout: &mut Layout<'_>,
    row: &[Cell],
    grid: Grid<'_>,
    header: bool,
) -> Result<(), PageCapExceeded> {
    let lines = wrapped(row, grid, header);
    let (tallest, full_height) = height_of(&lines);
    if layout.available() < full_height && !layout.page_is_blank() {
        layout.new_page()?;
        if !header {
            for repeated in grid.header {
                draw_row(layout, repeated, &wrapped(repeated, grid, true), grid, true)?;
            }
        }
    }
    let fitting = whole((layout.available() - 2.0 * CELL_PAD) / CELL_LEADING).max(1);
    let cut: Vec<Vec<Line>> = lines
        .into_iter()
        .map(|cell| cell.into_iter().take(tallest.min(fitting)).collect())
        .collect();
    draw_row(layout, row, &cut, grid, header)
}

fn draw_row(
    layout: &mut Layout<'_>,
    row: &[Cell],
    lines: &[Vec<Line>],
    grid: Grid<'_>,
    header: bool,
) -> Result<(), PageCapExceeded> {
    let (_, height) = height_of(lines);
    let top = layout.y();
    let bottom = top - height;
    let columns = grid.end - grid.start;
    let right = MARGIN + grid.width * lines_f32(columns);
    let page = layout.page_mut();
    page.rules.push(rule((MARGIN, top), (right, top)));
    for (offset, cell_lines) in lines.iter().enumerate() {
        let x = MARGIN + grid.width * lines_f32(offset);
        let shade = row
            .get(grid.start + offset)
            .and_then(|cell| cell.shade)
            .or(header.then_some(HEADER_TINT));
        if let Some(color) = shade {
            layout.page_mut().fills.push(Fill {
                rect: Rect {
                    x,
                    y: bottom,
                    w: grid.width,
                    h: height,
                },
                color,
            });
        }
        layout.page_mut().rules.push(rule((x, top), (x, bottom)));
        for (line_index, line) in cell_lines.iter().enumerate() {
            let baseline = top - CELL_PAD - CELL_SIZE - CELL_LEADING * lines_f32(line_index);
            layout.draw_line(line, x + CELL_PAD, baseline, measure(grid.width, header));
        }
    }
    layout
        .page_mut()
        .rules
        .push(rule((right, top), (right, bottom)));
    layout.advance(height);
    Ok(())
}
