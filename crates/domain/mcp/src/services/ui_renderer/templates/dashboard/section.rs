use super::super::html::html_escape;
use super::rand_id;
use serde_json::Value as JsonValue;

#[derive(Debug)]
pub(super) struct DashboardSection {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) section_type: SectionType,
    pub(super) data: JsonValue,
    width: Option<String>,
}

#[derive(Debug)]
pub(super) enum SectionType {
    Metrics,
    Chart,
    Table,
    Status,
    List,
    Text,
}

impl DashboardSection {
    pub(super) fn from_json(value: &JsonValue) -> Self {
        let title = value
            .get("title")
            .and_then(JsonValue::as_str)
            .unwrap_or("Section")
            .to_string();

        let section_type =
            value
                .get("type")
                .and_then(JsonValue::as_str)
                .map_or(SectionType::Text, |s| match s.to_lowercase().as_str() {
                    "metrics" | "kpi" => SectionType::Metrics,
                    "chart" | "graph" => SectionType::Chart,
                    "table" => SectionType::Table,
                    "status" => SectionType::Status,
                    "list" => SectionType::List,
                    _ => SectionType::Text,
                });

        let id = value
            .get("id")
            .and_then(JsonValue::as_str)
            .map_or_else(|| format!("section-{}", rand_id()), String::from);

        Self {
            id,
            title,
            section_type,
            data: value.clone(),
            width: value
                .get("width")
                .and_then(JsonValue::as_str)
                .map(String::from),
        }
    }

    pub(super) fn render_html(&self) -> String {
        let content = match &self.section_type {
            SectionType::Metrics => self.render_metrics(),
            SectionType::Chart => self.render_chart(),
            SectionType::Table => self.render_table(),
            SectionType::Status => self.render_status(),
            SectionType::List => self.render_list(),
            SectionType::Text => self.render_text(),
        };

        let width_style = self.width.as_ref().map_or_else(String::new, |w| {
            format!(r#" style="flex-basis: {}""#, html_escape(w))
        });

        format!(
            r#"<div class="dashboard-section" id="{id}"{width}>
    <h2 class="section-title">{title}</h2>
    <div class="section-content">
        {content}
    </div>
</div>"#,
            id = html_escape(&self.id),
            width = width_style,
            title = html_escape(&self.title),
            content = content,
        )
    }

    fn render_metrics(&self) -> String {
        let metrics = self
            .data
            .get("metrics")
            .or_else(|| self.data.get("data"))
            .and_then(JsonValue::as_array)
            .map_or_else(String::new, |arr| {
                arr.iter()
                    .filter_map(|m| {
                        let label = m.get("label").or_else(|| m.get("name")).and_then(JsonValue::as_str)?;
                        let value = m.get("value").map(|v| {
                            v.as_f64().map_or_else(
                                || v.to_string().trim_matches('"').to_string(),
                                |n| format!("{:.2}", n),
                            )
                        })?;
                        let change = m.get("change").and_then(JsonValue::as_f64);
                        let unit = m.get("unit").and_then(JsonValue::as_str).unwrap_or("");

                        let change_html = change
                            .map_or_else(String::new, |c| {
                                let class = if c >= 0.0 { "positive" } else { "negative" };
                                let sign = if c >= 0.0 { "+" } else { "" };
                                format!(r#"<span class="metric-change {class}">{sign}{c:.1}%</span>"#)
                            });

                        Some(format!(
                            r#"<div class="metric-card">
                                <div class="metric-value">{value}<span class="metric-unit">{unit}</span></div>
                                <div class="metric-label">{label}</div>
                                {change}
                            </div>"#,
                            value = html_escape(&value),
                            unit = html_escape(unit),
                            label = html_escape(label),
                            change = change_html,
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });

        format!(r#"<div class="metrics-grid">{}</div>"#, metrics)
    }

    fn render_chart(&self) -> String {
        format!(
            r#"<div class="chart-container"><canvas id="chart-{}"></canvas></div>"#,
            html_escape(&self.id)
        )
    }

    fn render_table(&self) -> String {
        use std::fmt::Write;

        let columns = self
            .data
            .get("columns")
            .and_then(JsonValue::as_array)
            .map_or_else(Vec::new, |arr| {
                arr.iter().filter_map(|c| c.as_str()).collect::<Vec<_>>()
            });

        let rows = self
            .data
            .get("rows")
            .or_else(|| self.data.get("data"))
            .and_then(JsonValue::as_array);

        if columns.is_empty() {
            return "<p>No table data</p>".to_string();
        }

        let header = columns.iter().fold(String::new(), |mut acc, c| {
            let _ = write!(acc, "<th>{}</th>", html_escape(c));
            acc
        });

        let body = rows.map_or_else(String::new, |arr| {
            arr.iter()
                .map(|row| {
                    let cells = row.as_object().map_or_else(
                        || {
                            row.as_array().map_or_else(Vec::new, |arr| {
                                arr.iter().map(ToString::to_string).collect()
                            })
                        },
                        |obj| {
                            columns
                                .iter()
                                .map(|c| obj.get(*c).map_or_else(String::new, ToString::to_string))
                                .collect::<Vec<_>>()
                        },
                    );

                    let cells_html = cells.iter().fold(String::new(), |mut acc, c| {
                        let _ = write!(acc, "<td>{}</td>", html_escape(c.trim_matches('"')));
                        acc
                    });

                    format!("<tr>{cells_html}</tr>")
                })
                .fold(String::new(), |mut acc, row| {
                    acc.push_str(&row);
                    acc
                })
        });

        format!(
            r#"<table class="section-table">
                <thead><tr>{header}</tr></thead>
                <tbody>{body}</tbody>
            </table>"#,
            header = header,
            body = body,
        )
    }

    fn render_status(&self) -> String {
        let items = self
            .data
            .get("items")
            .or_else(|| self.data.get("data"))
            .and_then(JsonValue::as_array)
            .map_or_else(String::new, |arr| {
                arr.iter()
                    .filter_map(|item| {
                        let name = item
                            .get("name")
                            .or_else(|| item.get("label"))
                            .and_then(JsonValue::as_str)?;
                        let status = item
                            .get("status")
                            .and_then(JsonValue::as_str)
                            .unwrap_or("unknown");
                        let status_class = match status.to_lowercase().as_str() {
                            "ok" | "healthy" | "success" | "active" => "status-ok",
                            "warning" | "degraded" => "status-warning",
                            "error" | "failed" | "critical" => "status-error",
                            _ => "status-unknown",
                        };

                        Some(format!(
                            r#"<div class="status-item">
                                <span class="status-indicator {class}"></span>
                                <span class="status-name">{name}</span>
                                <span class="status-value">{status}</span>
                            </div>"#,
                            class = status_class,
                            name = html_escape(name),
                            status = html_escape(status),
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });

        format!(r#"<div class="status-list">{}</div>"#, items)
    }

    fn render_list(&self) -> String {
        let items = self
            .data
            .get("items")
            .or_else(|| self.data.get("data"))
            .and_then(JsonValue::as_array)
            .map_or_else(String::new, |arr| {
                arr.iter()
                    .filter_map(|item| {
                        let text = if let Some(s) = item.as_str() {
                            s.to_string()
                        } else {
                            item.get("text")
                                .or_else(|| item.get("title"))
                                .and_then(JsonValue::as_str)
                                .map(String::from)?
                        };
                        Some(format!("<li>{}</li>", html_escape(&text)))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });

        format!(r#"<ul class="section-list">{}</ul>"#, items)
    }

    fn render_text(&self) -> String {
        let text = self
            .data
            .get("text")
            .or_else(|| self.data.get("content"))
            .and_then(JsonValue::as_str)
            .unwrap_or("");

        format!(r#"<p class="section-text">{}</p>"#, html_escape(text))
    }
}
