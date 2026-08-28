//! Table section body rendering and cell formatting.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::super::html::html_escape;
use super::{section_data, section_empty};
use crate::error::McpDomainResult;
use serde_json::Value as JsonValue;
use systemprompt_models::artifacts::dashboard::{DashboardSection, TableSectionData};

fn format_cell(value: Option<&JsonValue>) -> String {
    match value {
        None | Some(JsonValue::Null) => String::new(),
        Some(JsonValue::String(s)) => s.clone(),
        Some(JsonValue::Bool(b)) => if *b { "Yes" } else { "No" }.to_owned(),
        Some(JsonValue::Number(n)) => n.as_f64().map_or_else(|| n.to_string(), format_number),
        Some(other) => other.to_string(),
    }
}

fn format_number(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON && value.abs() < 1e15 {
        let whole = value.trunc() as i64;
        let digits = whole.abs().to_string();
        let mut grouped = String::new();
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                grouped.push(',');
            }
            grouped.push(ch);
        }
        if whole < 0 {
            format!("-{grouped}")
        } else {
            grouped
        }
    } else {
        format!("{value:.2}")
    }
}

fn row_cells(row: &JsonValue, columns: &[String]) -> Vec<String> {
    row.as_object().map_or_else(
        || {
            row.as_array().map_or_else(Vec::new, |arr| {
                arr.iter().map(|v| format_cell(Some(v))).collect()
            })
        },
        |obj| {
            columns
                .iter()
                .map(|c| format_cell(obj.get(c.as_str())))
                .collect()
        },
    )
}

pub(super) fn render_table(section: &DashboardSection) -> McpDomainResult<String> {
    let data: TableSectionData = section_data(section)?;

    if data.rows.is_empty() {
        return Ok(section_empty("No rows to show."));
    }

    // Why: `default_sort` was declared by the model and never applied. Sorting here
    // rather than client-side keeps the no-JS rendering correct too.
    let mut rows: Vec<&JsonValue> = data.rows.iter().collect();
    if let Some(sort) = &data.default_sort
        && let Some(index) = data.columns.iter().position(|c| c == &sort.column)
    {
        let descending = sort.order.eq_ignore_ascii_case("desc");
        rows.sort_by(|a, b| {
            let av = row_cells(a, &data.columns)
                .get(index)
                .cloned()
                .unwrap_or_default();
            let bv = row_cells(b, &data.columns)
                .get(index)
                .cloned()
                .unwrap_or_default();
            let ordering = match (av.parse::<f64>(), bv.parse::<f64>()) {
                (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                _ => av.cmp(&bv),
            };
            if descending {
                ordering.reverse()
            } else {
                ordering
            }
        });
    }

    let sortable = data.sortable.unwrap_or(false);
    let header = data.columns.iter().fold(String::new(), |mut acc, c| {
        acc.push_str(&format!(
            r#"<th scope="col"{sortable_attrs}>{label}</th>"#,
            sortable_attrs = if sortable {
                r#" class="sortable" tabindex="0" role="button""#
            } else {
                ""
            },
            label = html_escape(c),
        ));
        acc
    });

    let body = rows.iter().fold(String::new(), |mut acc, row| {
        let cells_html =
            row_cells(row, &data.columns)
                .iter()
                .fold(String::new(), |mut cells, c| {
                    cells.push_str(&format!("<td>{}</td>", html_escape(c)));
                    cells
                });
        acc.push_str(&format!("<tr>{cells_html}</tr>"));
        acc
    });

    Ok(format!(
        r#"<div class="section-table-wrap">
                <table class="section-table{sortable_class}">
                    <thead><tr>{header}</tr></thead>
                    <tbody>{body}</tbody>
                </table>
            </div>"#,
        sortable_class = if sortable {
            " section-table-sortable"
        } else {
            ""
        },
    ))
}
