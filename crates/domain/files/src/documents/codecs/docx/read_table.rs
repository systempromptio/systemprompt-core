//! A `w:tbl` read as an editable table block.
//!
//! The grid comes from `w:tblGrid`; a cell spanning columns (`w:gridSpan`)
//! keeps its slot followed by empty covered slots, and a vertical merge
//! (`w:vMerge`) counts its continuation cells into the restarting cell's
//! `rowspan`. Leading rows marked `w:tblHeader` are header rows. A cell
//! whose paragraphs all share one alignment carries it as the cell's
//! alignment. A table holding another table reads as `None` and its caller
//! keeps it as a `Raw` block.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::codecs::xml_tree::Element;
use crate::documents::model::layout::{CellProps, Column, TableProps, VAlign};
use crate::documents::model::{Block, BlockKind, RichText};

use super::read::{ReadCtx, paragraph, points};

pub(super) fn table(xml: &str, tbl: &Element, ctx: &ReadCtx) -> Option<BlockKind> {
    if tbl
        .children_named("w:tr")
        .any(|tr| tr.find("w:tbl").is_some())
    {
        return None;
    }
    let mut props = TableProps {
        style: tbl
            .child("w:tblPr")
            .and_then(|p| p.child("w:tblStyle"))
            .and_then(|s| s.attr("w:val"))
            .map(str::to_owned),
        columns: tbl
            .child("w:tblGrid")
            .map(|grid| {
                grid.children_named("w:gridCol")
                    .map(|col| Column {
                        width: col.attr("w:w").and_then(points),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        ..TableProps::default()
    };
    let mut rows: Vec<Vec<RichText>> = Vec::new();
    let mut cells: Vec<Vec<CellProps>> = Vec::new();
    let mut header = true;
    for tr in tbl.children_named("w:tr") {
        let is_header = tr
            .child("w:trPr")
            .and_then(|p| p.child("w:tblHeader"))
            .is_some();
        header &= is_header;
        if header {
            props.header_rows += 1;
        }
        let (mut row, mut styles) = (Vec::new(), Vec::new());
        for tc in tr.children_named("w:tc") {
            let tc_pr = tc.child("w:tcPr");
            let span = tc_pr
                .and_then(|p| p.child("w:gridSpan"))
                .and_then(|s| s.attr("w:val"))
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(1)
                .max(1);
            let merge = tc_pr.and_then(|p| p.child("w:vMerge"));
            let column = row.len();
            if merge.is_some_and(|m| m.attr("w:val") != Some("restart")) {
                extend_rowspan(&mut cells, column);
                row.push(RichText::default());
                styles.push(CellProps::default());
            } else {
                let (content, style) = cell(xml, tc, tc_pr, ctx);
                row.push(content);
                styles.push(CellProps {
                    colspan: if span > 1 { span } else { 0 },
                    rowspan: u32::from(merge.is_some()),
                    ..style
                });
            }
            for _ in 1..span {
                row.push(RichText::default());
                styles.push(CellProps::default());
            }
        }
        rows.push(row);
        cells.push(styles);
    }
    for cell in cells.iter_mut().flatten() {
        if cell.rowspan == 1 {
            cell.rowspan = 0;
        }
    }
    if cells.iter().flatten().any(|c| *c != CellProps::default()) {
        props.cells = cells;
    }
    Some(BlockKind::Table { rows, props })
}

fn extend_rowspan(cells: &mut [Vec<CellProps>], column: usize) {
    for row in cells.iter_mut().rev() {
        if let Some(cell) = row.get_mut(column)
            && cell.rowspan >= 1
        {
            cell.rowspan = cell.rowspan.saturating_add(1);
            return;
        }
    }
}

fn cell(xml: &str, tc: &Element, tc_pr: Option<&Element>, ctx: &ReadCtx) -> (RichText, CellProps) {
    let mut blocks: Vec<Block> = tc
        .children_named("w:p")
        .map(|p| paragraph(xml, p, ctx).unwrap_or_else(|| raw(xml, p)))
        .collect();
    let first = blocks.first().and_then(|b| b.attrs.align);
    let shared = first.filter(|_| blocks.iter().all(|b| b.attrs.align == first));
    if shared.is_some() {
        for block in &mut blocks {
            block.attrs.align = None;
        }
    }
    let props = CellProps {
        shade: tc_pr
            .and_then(|p| p.child("w:shd"))
            .and_then(|s| s.attr("w:fill"))
            .filter(|fill| *fill != "auto")
            .map(str::to_owned),
        align: shared,
        valign: tc_pr
            .and_then(|p| p.child("w:vAlign"))
            .and_then(|v| v.attr("w:val"))
            .and_then(|v| match v {
                "top" => Some(VAlign::Top),
                "center" => Some(VAlign::Center),
                "bottom" => Some(VAlign::Bottom),
                _ => None,
            }),
        ..CellProps::default()
    };
    (RichText::of(blocks), props)
}

pub(super) fn raw(xml: &str, element: &Element) -> Block {
    let source = element.raw(xml).to_owned();
    Block::new(String::new(), BlockKind::Raw { source }, "")
}
