//! Server-rendered SVG charts.
//!
//! Charts are emitted as inline SVG rather than driven by a charting library
//! for one reason that outranks aesthetics: a rendered artifact runs in a
//! sandboxed frame with an opaque origin under a strict CSP, where a
//! script-injected CDN bundle is exactly the thing that cannot be relied on.
//! Vector output needs no network, no script, and no `'unsafe-inline'`
//! widening, and it stays sharp at whatever size the host frame settles on.
//!
//! [`ChartSpec`] is the neutral input both a standalone `ChartArtifact` and a
//! dashboard's `ChartSectionData` map onto, so the two callers cannot drift.
//! Series colour comes from the `--mcpui-series-*` custom properties, which
//! means a registered [`ArtifactTheme`](super::super::ArtifactTheme) repaints
//! every chart without this module knowing a palette exists.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cartesian;
mod frame;
mod radial;
mod scale;

use super::html::html_escape;
use systemprompt_models::artifacts::chart::ChartDataset;
use systemprompt_models::artifacts::types::{AxisType, ChartType};

pub(super) const VIEW_W: f64 = 720.0;
pub(super) const VIEW_H: f64 = 340.0;
pub(super) const PAD_LEFT: f64 = 58.0;
pub(super) const PAD_RIGHT: f64 = 18.0;
pub(super) const PAD_TOP: f64 = 18.0;
pub(super) const PAD_BOTTOM: f64 = 46.0;
pub(super) const SERIES_SLOTS: usize = 6;

#[derive(Debug, Clone, Copy)]
pub struct ChartSpec<'a> {
    pub chart_type: ChartType,
    pub labels: &'a [String],
    pub datasets: &'a [ChartDataset],
    pub x_axis_label: &'a str,
    pub y_axis_label: &'a str,
    pub y_axis_type: AxisType,
}

impl ChartSpec<'_> {
    fn is_empty(&self) -> bool {
        self.datasets.iter().all(|set| set.data.is_empty())
    }
}

pub(super) fn series_color(index: usize) -> String {
    format!("var(--mcpui-series-{})", index % SERIES_SLOTS + 1)
}

pub fn render(spec: &ChartSpec<'_>, aria_label: &str) -> String {
    let plot = if spec.is_empty() {
        empty_plot()
    } else {
        match spec.chart_type {
            ChartType::Pie | ChartType::Doughnut => radial::plot(spec).unwrap_or_else(empty_plot),
            ChartType::Line | ChartType::Area | ChartType::Bar => cartesian::plot(spec),
        }
    };

    format!(
        r#"<figure class="chart">
    <svg class="chart-svg" viewBox="0 0 {VIEW_W} {VIEW_H}" preserveAspectRatio="xMidYMid meet" role="img" aria-labelledby="chart-title" aria-describedby="chart-desc">
        <title id="chart-title">{aria}</title>
        <desc id="chart-desc">{desc}</desc>
{plot}
    </svg>
{legend}
{table}
</figure>"#,
        aria = html_escape(aria_label),
        desc = html_escape(&describe(spec)),
        legend = legend(spec.datasets, spec.labels, spec.chart_type),
        table = data_table(spec),
    )
}

fn empty_plot() -> String {
    format!(
        r#"        <text class="chart-empty" x="{x}" y="{y}">No data to plot</text>"#,
        x = VIEW_W / 2.0,
        y = VIEW_H / 2.0,
    )
}

fn legend(datasets: &[ChartDataset], labels: &[String], chart_type: ChartType) -> String {
    let names: Vec<&str> = match chart_type {
        ChartType::Pie | ChartType::Doughnut => labels.iter().map(String::as_str).collect(),
        ChartType::Line | ChartType::Area | ChartType::Bar => {
            datasets.iter().map(|set| set.label.as_str()).collect()
        },
    };

    // Why: A one-series chart used to render no legend at all, so its dataset's
    // own label appeared nowhere in the document.
    if names.is_empty() || names.iter().all(|n| n.is_empty()) {
        return String::new();
    }

    let items = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            format!(
                r#"<span class="chart-legend-item"><span class="chart-swatch" style="--chart-swatch-color: {color}"></span>{name}</span>"#,
                color = series_color(i),
                name = html_escape(name),
            )
        })
        .collect::<Vec<_>>()
        .join("\n        ");

    format!(
        r#"    <figcaption class="chart-legend">
        {items}
    </figcaption>"#
    )
}

fn describe(spec: &ChartSpec<'_>) -> String {
    let series = spec.datasets.len();
    let points: usize = spec.datasets.iter().map(|set| set.data.len()).sum();
    let values: Vec<f64> = spec
        .datasets
        .iter()
        .flat_map(|set| set.data.iter().copied())
        .filter(|v| v.is_finite())
        .collect();

    let range = if values.is_empty() {
        String::new()
    } else {
        let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        format!(
            ", ranging from {} to {}",
            scale::format_value(lo),
            scale::format_value(hi)
        )
    };

    format!(
        "{kind} chart with {series} series and {points} data points{range}.",
        kind = match spec.chart_type {
            ChartType::Line => "Line",
            ChartType::Bar => "Bar",
            ChartType::Pie => "Pie",
            ChartType::Doughnut => "Doughnut",
            ChartType::Area => "Area",
        },
    )
}

fn data_table(spec: &ChartSpec<'_>) -> String {
    if spec.datasets.is_empty() {
        return String::new();
    }

    let head = spec
        .datasets
        .iter()
        .map(|set| format!("<th scope=\"col\">{}</th>", html_escape(&set.label)))
        .collect::<Vec<_>>()
        .concat();

    let rows: usize = spec
        .datasets
        .iter()
        .map(|set| set.data.len())
        .max()
        .unwrap_or(0);
    let body = (0..rows)
        .map(|r| {
            let label = spec
                .labels
                .get(r)
                .map_or_else(|| format!("Point {}", r + 1), |l| html_escape(l));
            let cells = spec
                .datasets
                .iter()
                .map(|set| {
                    set.data.get(r).map_or_else(
                        || "<td></td>".to_owned(),
                        |v| format!("<td>{}</td>", scale::format_value(*v)),
                    )
                })
                .collect::<Vec<_>>()
                .concat();
            format!("<tr><th scope=\"row\">{label}</th>{cells}</tr>")
        })
        .collect::<Vec<_>>()
        .concat();

    format!(
        r#"    <table class="visually-hidden"><caption>Chart data</caption><thead><tr><th scope="col">Label</th>{head}</tr></thead><tbody>{body}</tbody></table>"#
    )
}
