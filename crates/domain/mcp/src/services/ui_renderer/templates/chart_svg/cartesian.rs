//! Bar, line, and area plots on a shared category/linear frame.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::frame::{self, Frame};
use super::{
    ChartSpec, PAD_BOTTOM, PAD_LEFT, PAD_RIGHT, PAD_TOP, VIEW_H, VIEW_W, scale, series_color,
};
use crate::services::ui_renderer::templates::html::html_escape;
use systemprompt_models::artifacts::chart::ChartDataset;
use systemprompt_models::artifacts::types::ChartType;

const BAND_INSET: f64 = 0.18;

pub(super) fn plot(spec: &ChartSpec<'_>) -> String {
    let slots = spec
        .datasets
        .iter()
        .map(|set| set.data.len())
        .max()
        .unwrap_or(0)
        .max(spec.labels.len())
        .max(1);

    let width = VIEW_W - PAD_LEFT - PAD_RIGHT;
    let height = VIEW_H - PAD_TOP - PAD_BOTTOM;
    let frame = Frame {
        left: PAD_LEFT,
        top: PAD_TOP,
        width,
        height,
        scale: scale::linear(spec.datasets),
        band: width / slots as f64,
    };

    let series = match spec.chart_type {
        ChartType::Bar => bars(spec.datasets, frame),
        ChartType::Line | ChartType::Area | ChartType::Pie | ChartType::Doughnut => lines(
            spec.datasets,
            frame,
            matches!(spec.chart_type, ChartType::Area),
        ),
    };

    format!(
        "{grid}\n{axes}\n{ticks}\n{series}",
        grid = frame::gridlines(frame),
        axes = frame::axis_titles(spec),
        ticks = frame::ticks(spec.labels, frame, slots),
    )
}

fn bars(datasets: &[ChartDataset], frame: Frame) -> String {
    let count = datasets.len().max(1);
    let group = frame.band * BAND_INSET.mul_add(-2.0, 1.0);
    let width = group / count as f64;
    let zero = frame.y_of(0.0);
    let label_values = datasets.len() == 1 && frame.band > 34.0;

    datasets
        .iter()
        .enumerate()
        .map(|(series, set)| {
            let bars = set
                .data
                .iter()
                .enumerate()
                .map(|(i, &value)| {
                    one_bar(
                        BarAt {
                            frame,
                            i,
                            series,
                            count,
                            width,
                            zero,
                        },
                        value,
                        label_values,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "        <g class=\"chart-series\" fill=\"{color}\">\n{bars}\n        </g>",
                color = series_color(series),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone, Copy)]
struct BarAt {
    frame: Frame,
    i: usize,
    series: usize,
    count: usize,
    width: f64,
    zero: f64,
}

fn one_bar(at: BarAt, value: f64, label: bool) -> String {
    let group_left = at.width.mul_add(
        at.series as f64,
        at.frame.band_center(at.i) - (at.width * at.count as f64) / 2.0,
    );
    let y = at.frame.y_of(value);
    let top = y.min(at.zero);
    let height = (y - at.zero).abs().max(1.0);
    let inset = (at.width * 0.12).min(3.0);

    let caption = if label {
        format!(
            "\n{}",
            format_args!(
                r#"            <text class="chart-value" x="{x}" y="{ty}">{text}</text>"#,
                x = scale::coord(group_left + at.width / 2.0),
                ty = scale::coord(top - 5.0),
                text = html_escape(&scale::format_value(value)),
            )
        )
    } else {
        String::new()
    };

    format!(
        r#"            <rect class="chart-bar" x="{x}" y="{y}" width="{w}" height="{h}" rx="2"><title>{title}</title></rect>{caption}"#,
        x = scale::coord(group_left + inset / 2.0),
        y = scale::coord(top),
        w = scale::coord((at.width - inset).max(1.0)),
        h = scale::coord(height),
        title = html_escape(&scale::format_value(value)),
    )
}

fn lines(datasets: &[ChartDataset], frame: Frame, fill_area: bool) -> String {
    datasets
        .iter()
        .enumerate()
        .map(|(series, set)| {
            let color = series_color(series);
            let points: Vec<(f64, f64)> = set
                .data
                .iter()
                .enumerate()
                .map(|(i, &value)| (frame.band_center(i), frame.y_of(value)))
                .collect();

            let path = points
                .iter()
                .map(|(x, y)| format!("{} {}", scale::coord(*x), scale::coord(*y)))
                .collect::<Vec<_>>()
                .join(" L ");

            let area = if fill_area && points.len() > 1 {
                let (first_x, _) = points[0];
                let (last_x, _) = points[points.len() - 1];
                let base = scale::coord(frame.y_of(frame.scale.min));
                format!(
                    "\n            <path class=\"chart-area\" fill=\"{color}\" d=\"M {first} {base} L {path} L {last} {base} Z\" />",
                    first = scale::coord(first_x),
                    last = scale::coord(last_x),
                )
            } else {
                String::new()
            };

            let dots = points
                .iter()
                .zip(set.data.iter())
                .map(|((x, y), value)| {
                    format!(
                        r#"            <circle class="chart-point" cx="{cx}" cy="{cy}" r="3.5" fill="{color}"><title>{title}</title></circle>"#,
                        cx = scale::coord(*x),
                        cy = scale::coord(*y),
                        title = html_escape(&scale::format_value(*value)),
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            format!(
                "        <g class=\"chart-series\">{area}\n            <path class=\"chart-line\" stroke=\"{color}\" d=\"M {path}\" />\n{dots}\n        </g>"
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
