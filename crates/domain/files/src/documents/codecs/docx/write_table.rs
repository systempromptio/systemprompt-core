//! A table block written as a `w:tbl`.
//!
//! The grid has one `w:gridCol` per column, with its width when the model
//! gives one (then the layout is fixed). A cell spanning columns writes one
//! `w:tc` with `w:gridSpan` and skips the slots it covers; a cell spanning
//! rows writes `w:vMerge="restart"` and each row below gets a continuation
//! cell. Leading header rows repeat on every page (`w:tblHeader`). A table
//! with no style gets single borders so it reads as a table.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::markdown::BORDERS;
use super::write::{WriteCtx, escape_attr, hex, write_in_cell};
use crate::documents::model::layout::{CellProps, TableProps, VAlign};
use crate::documents::model::{Block, RichText};

const LOOK: &str = "<w:tblLook w:val=\"04A0\" w:firstRow=\"1\" w:lastRow=\"0\" \
    w:firstColumn=\"1\" w:lastColumn=\"0\" w:noHBand=\"0\" w:noVBand=\"1\"/>";

pub(super) fn table(rows: &[Vec<RichText>], props: &TableProps, ctx: &mut WriteCtx<'_>) -> String {
    let columns = rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(props.columns.len())
        .max(1);
    let widths: Vec<Option<u32>> = (0..columns)
        .map(|c| props.columns.get(c).and_then(|col| col.width))
        .collect();
    let mut out = head(props, &widths);
    let mut remaining = vec![0u32; columns];
    let mut spans = vec![1u32; columns];
    for (r, row) in rows.iter().enumerate() {
        out.push_str("<w:tr>");
        if u32::try_from(r).is_ok_and(|r| r < props.header_rows) {
            out.push_str("<w:trPr><w:tblHeader/></w:trPr>");
        }
        let mut c = 0;
        while c < columns {
            if remaining[c] > 0 {
                let span = spans[c].max(1);
                for slot in remaining.iter_mut().skip(c).take(span as usize) {
                    *slot = slot.saturating_sub(1);
                }
                let width = tc_width(&widths, c, span);
                let grid = grid_span(span);
                out.push_str(&format!(
                    "<w:tc><w:tcPr>{width}{grid}<w:vMerge/></w:tcPr><w:p/></w:tc>"
                ));
                c += span as usize;
                continue;
            }
            let cell = props.cell(r, c);
            let span = cell
                .colspan()
                .min(u32::try_from(columns - c).unwrap_or(1))
                .max(1);
            if cell.rowspan() > 1 {
                for slot in remaining.iter_mut().skip(c).take(span as usize) {
                    *slot = cell.rowspan() - 1;
                }
                spans[c] = span;
            }
            let content = row
                .get(c)
                .map_or(&[][..], |content| content.blocks.as_slice());
            out.push_str(&origin_cell(
                &cell,
                content,
                tc_width(&widths, c, span),
                span,
                ctx,
            ));
            c += span as usize;
        }
        out.push_str("</w:tr>");
    }
    out.push_str("</w:tbl>");
    out
}

fn head(props: &TableProps, widths: &[Option<u32>]) -> String {
    let mut out = String::from("<w:tbl><w:tblPr>");
    if let Some(style) = &props.style {
        out.push_str(&format!("<w:tblStyle w:val=\"{}\"/>", escape_attr(style)));
    }
    out.push_str("<w:tblW w:w=\"0\" w:type=\"auto\"/>");
    if props.style.is_none() {
        out.push_str(BORDERS);
    }
    if widths.iter().all(Option::is_some) {
        out.push_str("<w:tblLayout w:type=\"fixed\"/>");
    }
    out.push_str(LOOK);
    out.push_str("</w:tblPr><w:tblGrid>");
    for width in widths {
        match width {
            Some(points) => {
                let twips = points.saturating_mul(20);
                out.push_str(&format!("<w:gridCol w:w=\"{twips}\"/>"));
            },
            None => out.push_str("<w:gridCol/>"),
        }
    }
    out.push_str("</w:tblGrid>");
    out
}

fn origin_cell(
    cell: &CellProps,
    content: &[Block],
    width: String,
    span: u32,
    ctx: &mut WriteCtx<'_>,
) -> String {
    let mut tc_pr = width + &grid_span(span);
    if cell.rowspan() > 1 {
        tc_pr.push_str("<w:vMerge w:val=\"restart\"/>");
    }
    if let Some(fill) = cell.shade.as_deref().and_then(hex) {
        tc_pr.push_str(&format!(
            "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{fill}\"/>"
        ));
    }
    if let Some(valign) = cell.valign {
        tc_pr.push_str(&format!("<w:vAlign w:val=\"{}\"/>", valign_name(valign)));
    }
    let mut body: String = content
        .iter()
        .map(|block| write_in_cell(block, ctx, cell.align))
        .collect();
    if body.is_empty() {
        body.push_str("<w:p/>");
    }
    format!("<w:tc><w:tcPr>{tc_pr}</w:tcPr>{body}</w:tc>")
}

fn tc_width(widths: &[Option<u32>], from: usize, span: u32) -> String {
    let covered = widths.iter().skip(from).take(span as usize);
    let total = covered
        .map(|w| w.map(|points| points.saturating_mul(20)))
        .try_fold(0u32, |sum, twips| twips.map(|t| sum.saturating_add(t)));
    total.map_or_else(
        || "<w:tcW w:w=\"0\" w:type=\"auto\"/>".to_owned(),
        |twips| format!("<w:tcW w:w=\"{twips}\" w:type=\"dxa\"/>"),
    )
}

fn grid_span(span: u32) -> String {
    if span > 1 {
        format!("<w:gridSpan w:val=\"{span}\"/>")
    } else {
        String::new()
    }
}

const fn valign_name(valign: VAlign) -> &'static str {
    match valign {
        VAlign::Top => "top",
        VAlign::Center => "center",
        VAlign::Bottom => "bottom",
    }
}
