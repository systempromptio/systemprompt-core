use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table as RatatuiTable, Widget, Wrap};
use systemprompt_models::a2a::Artifact;

use super::text::extract_data_content;

pub fn render(artifact: &Artifact, area: Rect, buf: &mut Buffer, scroll_offset: usize) {
    let Some(data) = extract_data_content(&artifact.parts) else {
        render_no_data(artifact, area, buf);
        return;
    };

    let columns: Vec<TableColumn> = data
        .get("columns")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|col| {
                    let key = col.get("key").and_then(|v| v.as_str())?;
                    let label = col.get("label").and_then(|v| v.as_str()).unwrap_or(key);
                    let col_type = col.get("type").and_then(|v| v.as_str()).unwrap_or("string");
                    Some(TableColumn {
                        key: key.to_string(),
                        label: label.to_string(),
                        col_type: col_type.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    if columns.is_empty() {
        render_no_data(artifact, area, buf);
        return;
    }

    let items: Vec<&serde_json::Value> = data
        .get("items")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().collect())
        .unwrap_or_default();

    let header_cells: Vec<Cell> = columns
        .iter()
        .map(|col| {
            Cell::from(col.label.clone()).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
        })
        .collect();
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = items
        .iter()
        .skip(scroll_offset)
        .take(usize::from(area.height.saturating_sub(3)))
        .map(|item| {
            let cells: Vec<Cell> = columns
                .iter()
                .map(|col| {
                    let value = item.get(&col.key);
                    let formatted = format_cell_value(value, &col.col_type);
                    Cell::from(formatted)
                })
                .collect();
            Row::new(cells)
        })
        .collect();

    let col_count = columns.len();
    let percentage = u16::try_from(100 / col_count).unwrap_or(u16::MAX);
    let widths: Vec<Constraint> = (0..col_count)
        .map(|_| Constraint::Percentage(percentage))
        .collect();

    let title = artifact
        .name
        .as_ref()
        .map_or_else(|| " Table ".to_string(), |n| format!(" {} ", n));

    let table = RatatuiTable::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(title),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    Widget::render(table, area, buf);
}

fn render_no_data(artifact: &Artifact, area: Rect, buf: &mut Buffer) {
    let lines = vec![
        Line::from(format!(
            "Table: {}",
            artifact.name.as_deref().unwrap_or("Unnamed")
        )),
        Line::from(""),
        Line::from("No data available"),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    paragraph.render(area, buf);
}

fn format_cell_value(value: Option<&serde_json::Value>, col_type: &str) -> String {
    let Some(value) = value else {
        return "-".to_string();
    };

    match col_type {
        "currency" => value
            .as_f64()
            .map_or_else(|| value_to_string(value), |n| format!("${:.2}", n)),
        "percentage" => value
            .as_f64()
            .map_or_else(|| value_to_string(value), |n| format!("{:.1}%", n * 100.0)),
        "number" | "integer" => value.as_f64().map_or_else(
            || value_to_string(value),
            |n| {
                if n.fract() == 0.0 {
                    format!("{}", n as i64)
                } else {
                    format!("{:.2}", n)
                }
            },
        ),
        "boolean" => value.as_bool().map_or_else(
            || value_to_string(value),
            |b| if b { "✓" } else { "✗" }.to_string(),
        ),
        _ => value_to_string(value),
    }
}

fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "-".to_string(),
        _ => value.to_string(),
    }
}

struct TableColumn {
    key: String,
    label: String,
    col_type: String,
}
