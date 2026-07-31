//! The plotting frame shared by bar, line, and area charts, plus the axis
//! chrome drawn on it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::scale::{self, Scale};
use super::{ChartSpec, PAD_BOTTOM, PAD_LEFT, PAD_RIGHT, PAD_TOP, VIEW_H, VIEW_W};
use crate::services::ui_renderer::templates::html::html_escape;

#[derive(Debug, Clone, Copy)]
pub(super) struct Frame {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
    pub scale: Scale,
    pub band: f64,
}

impl Frame {
    pub(super) fn y_of(self, value: f64) -> f64 {
        self.height
            .mul_add(1.0 - self.scale.fraction(value), self.top)
    }

    pub(super) fn band_center(self, index: usize) -> f64 {
        self.band.mul_add(index as f64 + 0.5, self.left)
    }
}

pub(super) fn gridlines(frame: Frame) -> String {
    let lines = frame
        .scale
        .ticks()
        .into_iter()
        .map(|value| {
            let y = scale::coord(frame.y_of(value));
            format!(
                r#"            <line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" />"#,
                x1 = scale::coord(frame.left),
                x2 = scale::coord(frame.left + frame.width),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("        <g class=\"chart-grid\">\n{lines}\n        </g>")
}

pub(super) fn ticks(labels: &[String], frame: Frame, slots: usize) -> String {
    let value_ticks = frame
        .scale
        .ticks()
        .into_iter()
        .map(|value| {
            format!(
                r#"            <text class="chart-tick" x="{x}" y="{y}" text-anchor="end" dominant-baseline="middle">{label}</text>"#,
                x = scale::coord(frame.left - 10.0),
                y = scale::coord(frame.y_of(value)),
                label = html_escape(&scale::format_value(value)),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let stride = slots.div_ceil(12).max(1);
    let category_ticks = labels
        .iter()
        .enumerate()
        .filter(|(i, _)| i % stride == 0)
        .map(|(i, label)| {
            format!(
                r#"            <text class="chart-tick" x="{x}" y="{y}" text-anchor="middle">{label}</text>"#,
                x = scale::coord(frame.band_center(i)),
                y = scale::coord(frame.top + frame.height + 18.0),
                label = html_escape(label),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("{value_ticks}\n{category_ticks}")
}

pub(super) fn axis_titles(spec: &ChartSpec<'_>) -> String {
    let baseline = format!(
        r#"        <g class="chart-axis"><line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" /></g>"#,
        x1 = scale::coord(PAD_LEFT),
        x2 = scale::coord(VIEW_W - PAD_RIGHT),
        y = scale::coord(VIEW_H - PAD_BOTTOM),
    );

    let x_title = if spec.x_axis_label.is_empty() {
        String::new()
    } else {
        format!(
            "\n{}",
            format_args!(
                r#"        <text class="chart-axis-label" x="{x}" y="{y}" text-anchor="middle">{label}</text>"#,
                x = scale::coord(VIEW_W / 2.0),
                y = scale::coord(VIEW_H - 8.0),
                label = html_escape(spec.x_axis_label),
            )
        )
    };

    let y_title = if spec.y_axis_label.is_empty() {
        String::new()
    } else {
        format!(
            "\n{}",
            format_args!(
                r#"        <text class="chart-axis-label" transform="rotate(-90 14 {mid})" x="14" y="{mid}" text-anchor="middle">{label}</text>"#,
                mid = scale::coord(PAD_TOP + (VIEW_H - PAD_TOP - PAD_BOTTOM) / 2.0),
                label = html_escape(spec.y_axis_label),
            )
        )
    };

    format!("{baseline}{x_title}{y_title}")
}
