//! Cell writes for the `.ods` codec.
//!
//! Writes are collected for the whole batch and rendered once per row
//! element: a row or cell whose repeat run covers a written position is
//! split into the untouched run before it, the written row or cell, and the
//! untouched run after it, each carrying its own repeat count. Positions
//! past the last row or cell are reached by appending empty filler runs.
//! Every byte outside the split elements stays as it was.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_provider_contracts::documents::CellValue;

use super::package::{Splice, attr_escaped, text_markup};
use super::sheet::{CellSeg, RowSeg, Sheet};
use crate::documents::codecs::xml_tree::Element;

pub(super) type RowWrites = BTreeMap<u32, BTreeMap<u32, CellValue>>;

const VALUE_ATTRS: [&str; 10] = [
    "office:value-type",
    "office:value",
    "office:date-value",
    "office:time-value",
    "office:boolean-value",
    "office:string-value",
    "office:currency",
    "table:formula",
    "calcext:value-type",
    "table:number-columns-repeated",
];

pub(super) fn with_attr(tag: &str, name: &str, value: Option<&str>) -> String {
    let mut out = tag.to_owned();
    let needle = format!(" {name}=");
    if let Some(at) = out.find(&needle) {
        let value_start = at + needle.len();
        let quote = out[value_start..].chars().next().unwrap_or('"');
        let close = out[value_start + 1..]
            .find(quote)
            .map_or(out.len(), |i| value_start + 2 + i);
        out.replace_range(at..close, "");
    }
    if let Some(value) = value {
        let end = if out.ends_with("/>") {
            out.len() - 2
        } else {
            out.len() - 1
        };
        out.insert_str(end, &format!(" {name}=\"{}\"", attr_escaped(value)));
    }
    out
}

fn parts<'x>(xml: &'x str, element: &Element) -> (&'x str, &'x str, &'x str) {
    (
        &xml[element.start..element.inner_start],
        &xml[element.inner_start..element.inner_end],
        &xml[element.inner_end..element.end],
    )
}

fn repeated(xml: &str, element: &Element, attr: &str, count: u32) -> String {
    let (open, inner, close) = parts(xml, element);
    let value = (count > 1).then(|| count.to_string());
    format!("{}{inner}{close}", with_attr(open, attr, value.as_deref()))
}

fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn written_cell(xml: &str, base: Option<&CellSeg>, value: &CellValue) -> String {
    let mut tag = match base {
        Some(seg) if seg.element.name == "table:table-cell" => VALUE_ATTRS
            .iter()
            .fold(parts(xml, &seg.element).0.to_owned(), |tag, attr| {
                with_attr(&tag, attr, None)
            }),
        _ => "<table:table-cell>".to_owned(),
    };
    if tag.ends_with("/>") {
        tag = format!("{}>", tag.trim_end_matches("/>").trim_end());
    }
    let (attrs, lines): (Vec<(&str, String)>, Vec<String>) = match value {
        CellValue::Empty => (Vec::new(), Vec::new()),
        CellValue::Bool(flag) => (
            vec![
                ("office:value-type", "boolean".to_owned()),
                ("office:boolean-value", flag.to_string()),
            ],
            vec![if *flag { "TRUE" } else { "FALSE" }.to_owned()],
        ),
        CellValue::Number(n) => (
            vec![
                ("office:value-type", "float".to_owned()),
                ("office:value", number(*n)),
            ],
            vec![number(*n)],
        ),
        CellValue::Text(text) => match text.strip_prefix('=') {
            Some(formula) => (
                vec![("table:formula", format!("of:={formula}"))],
                Vec::new(),
            ),
            None => (
                vec![("office:value-type", "string".to_owned())],
                text.split('\n').map(str::to_owned).collect(),
            ),
        },
    };
    for (name, value) in attrs {
        tag = with_attr(&tag, name, Some(&value));
    }
    let body: String = lines
        .iter()
        .map(|line| format!("<text:p>{}</text:p>", text_markup(line)))
        .collect();
    format!("{tag}{body}</table:table-cell>")
}

fn filler_cells(count: u32) -> String {
    match count {
        0 => String::new(),
        1 => "<table:table-cell/>".to_owned(),
        n => format!("<table:table-cell table:number-columns-repeated=\"{n}\"/>"),
    }
}

fn written_cells(xml: &str, cells: &[CellSeg], writes: &BTreeMap<u32, CellValue>) -> String {
    let mut out = String::new();
    let mut pending: Vec<(&u32, &CellValue)> = writes.iter().collect();
    pending.reverse();
    let mut next_col = 1u32;
    for seg in cells {
        let end = seg.start_col.saturating_add(seg.repeat);
        let mut at = seg.start_col;
        while let Some((&col, value)) = pending.last().copied().filter(|(c, _)| **c < end) {
            pending.pop();
            if col > at {
                out.push_str(&repeated(
                    xml,
                    &seg.element,
                    "table:number-columns-repeated",
                    col - at,
                ));
            }
            out.push_str(&written_cell(xml, Some(seg), value));
            at = col + 1;
        }
        if at == seg.start_col {
            out.push_str(&xml[seg.element.start..seg.element.end]);
        } else if end > at {
            out.push_str(&repeated(
                xml,
                &seg.element,
                "table:number-columns-repeated",
                end - at,
            ));
        }
        next_col = end;
    }
    for (&col, value) in pending.into_iter().rev() {
        out.push_str(&filler_cells(col.saturating_sub(next_col)));
        out.push_str(&written_cell(xml, None, value));
        next_col = col + 1;
    }
    out
}

fn written_row(xml: &str, seg: Option<&RowSeg>, writes: &BTreeMap<u32, CellValue>) -> String {
    match seg {
        Some(seg) => {
            let open = with_attr(
                parts(xml, &seg.element).0,
                "table:number-rows-repeated",
                None,
            );
            let open = if open.ends_with("/>") {
                format!("{}>", open.trim_end_matches("/>").trim_end())
            } else {
                open
            };
            format!(
                "{open}{}</table:table-row>",
                written_cells(xml, &seg.cells, writes)
            )
        },
        None => format!(
            "<table:table-row>{}</table:table-row>",
            written_cells(xml, &[], writes)
        ),
    }
}

pub(super) fn render(xml: &str, sheet: &Sheet, writes: &RowWrites, order: usize) -> Vec<Splice> {
    let mut splices = Vec::new();
    let mut next_row = 1u32;
    for seg in &sheet.rows {
        let end = seg.start_row.saturating_add(seg.repeat);
        let hits: Vec<_> = writes.range(seg.start_row..end).collect();
        next_row = end;
        if hits.is_empty() {
            continue;
        }
        let mut out = String::new();
        let mut at = seg.start_row;
        for (&row, cells) in hits {
            if row > at {
                out.push_str(&repeated(
                    xml,
                    &seg.element,
                    "table:number-rows-repeated",
                    row - at,
                ));
            }
            out.push_str(&written_row(xml, Some(seg), cells));
            at = row + 1;
        }
        if end > at {
            out.push_str(&repeated(
                xml,
                &seg.element,
                "table:number-rows-repeated",
                end - at,
            ));
        }
        splices.push(Splice {
            range: seg.element.start..seg.element.end,
            text: out,
            order,
        });
    }
    let mut appended = String::new();
    for (&row, cells) in writes.range(next_row..) {
        let gap = row - next_row;
        if gap > 0 {
            let repeat = if gap > 1 {
                format!(" table:number-rows-repeated=\"{gap}\"")
            } else {
                String::new()
            };
            appended.push_str(&format!(
                "<table:table-row{repeat}><table:table-cell/></table:table-row>"
            ));
        }
        appended.push_str(&written_row(xml, None, cells));
        next_row = row + 1;
    }
    if !appended.is_empty() {
        splices.push(Splice {
            range: sheet.rows_end..sheet.rows_end,
            text: appended,
            order,
        });
    }
    splices
}
