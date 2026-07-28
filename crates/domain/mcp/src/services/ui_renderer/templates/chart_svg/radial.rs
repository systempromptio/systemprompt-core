//! Pie and doughnut plots.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ChartSpec, VIEW_H, VIEW_W, scale, series_color};
use crate::services::ui_renderer::templates::html::html_escape;
use systemprompt_models::artifacts::types::ChartType;

const DOUGHNUT_INNER: f64 = 0.58;

pub(super) fn plot(spec: &ChartSpec<'_>) -> String {
    // Why: a slice chart divides one whole, so only the first dataset is
    // meaningful — a second series would be a different whole on the same axis.
    let Some(values) = spec.datasets.first().map(|set| set.data.as_slice()) else {
        return String::new();
    };

    let total: f64 = values.iter().filter(|v| v.is_finite() && **v > 0.0).sum();
    if total <= 0.0 {
        return String::new();
    }

    let cx = VIEW_W / 2.0;
    let cy = VIEW_H / 2.0;
    let radius = (VIEW_H / 2.0 - 24.0).min(VIEW_W / 2.0 - 24.0);
    let inner = if matches!(spec.chart_type, ChartType::Doughnut) {
        radius * DOUGHNUT_INNER
    } else {
        0.0
    };

    let mut start = -std::f64::consts::FRAC_PI_2;
    let mut slices = Vec::with_capacity(values.len());

    for (i, &value) in values.iter().enumerate() {
        if !value.is_finite() || value <= 0.0 {
            continue;
        }
        let sweep = value / total * std::f64::consts::TAU;
        let end = start + sweep;
        let label = spec.labels.get(i).map_or("", String::as_str);
        slices.push(slice(
            Arc {
                cx,
                cy,
                radius,
                inner,
                start,
                end,
            },
            &series_color(i),
            &format!(
                "{label}: {value} ({pct:.0}%)",
                value = scale::format_value(value),
                pct = value / total * 100.0,
            ),
        ));
        start = end;
    }

    let center = if inner > 0.0 {
        format!(
            "\n{}",
            format_args!(
                r#"        <text class="chart-tick" x="{cx}" y="{cy}" text-anchor="middle" dominant-baseline="middle">{total}</text>"#,
                cx = scale::coord(cx),
                cy = scale::coord(cy),
                total = html_escape(&scale::format_value(total)),
            )
        )
    } else {
        String::new()
    };

    format!(
        "        <g class=\"chart-series\">\n{}\n        </g>{center}",
        slices.join("\n")
    )
}

#[derive(Debug, Clone, Copy)]
struct Arc {
    cx: f64,
    cy: f64,
    radius: f64,
    inner: f64,
    start: f64,
    end: f64,
}

fn slice(arc: Arc, color: &str, title: &str) -> String {
    let large = i32::from(arc.end - arc.start > std::f64::consts::PI);
    let (x0, y0) = point(arc.cx, arc.cy, arc.radius, arc.start);
    let (x1, y1) = point(arc.cx, arc.cy, arc.radius, arc.end);

    let d = if arc.inner > 0.0 {
        let (ix1, iy1) = point(arc.cx, arc.cy, arc.inner, arc.end);
        let (ix0, iy0) = point(arc.cx, arc.cy, arc.inner, arc.start);
        format!(
            "M {x0} {y0} A {r} {r} 0 {large} 1 {x1} {y1} L {ix1} {iy1} A {ri} {ri} 0 {large} 0 {ix0} {iy0} Z",
            x0 = scale::coord(x0),
            y0 = scale::coord(y0),
            x1 = scale::coord(x1),
            y1 = scale::coord(y1),
            ix0 = scale::coord(ix0),
            iy0 = scale::coord(iy0),
            ix1 = scale::coord(ix1),
            iy1 = scale::coord(iy1),
            r = scale::coord(arc.radius),
            ri = scale::coord(arc.inner),
        )
    } else {
        format!(
            "M {cx} {cy} L {x0} {y0} A {r} {r} 0 {large} 1 {x1} {y1} Z",
            cx = scale::coord(arc.cx),
            cy = scale::coord(arc.cy),
            x0 = scale::coord(x0),
            y0 = scale::coord(y0),
            x1 = scale::coord(x1),
            y1 = scale::coord(y1),
            r = scale::coord(arc.radius),
        )
    };

    format!(
        r#"            <path class="chart-slice" fill="{color}" d="{d}"><title>{title}</title></path>"#,
        title = html_escape(title),
    )
}

fn point(cx: f64, cy: f64, radius: f64, angle: f64) -> (f64, f64) {
    (
        radius.mul_add(angle.cos(), cx),
        radius.mul_add(angle.sin(), cy),
    )
}
