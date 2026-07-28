//! Axis range selection and number formatting for SVG charts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::artifacts::chart::ChartDataset;

pub(super) const TICK_COUNT: usize = 4;

#[derive(Debug, Clone, Copy)]
pub(super) struct Scale {
    pub min: f64,
    pub max: f64,
}

impl Scale {
    pub(super) fn fraction(self, value: f64) -> f64 {
        let span = self.max - self.min;
        if span <= 0.0 {
            return 0.0;
        }
        ((value - self.min) / span).clamp(0.0, 1.0)
    }

    pub(super) fn ticks(self) -> Vec<f64> {
        let step = (self.max - self.min) / TICK_COUNT as f64;
        (0..=TICK_COUNT)
            .map(|i| step.mul_add(i as f64, self.min))
            .collect()
    }
}

// Why: the baseline is pinned to zero whenever the data does not cross it — a
// bar chart drawn from a floating baseline exaggerates every difference on it.
pub(super) fn linear(datasets: &[ChartDataset]) -> Scale {
    let values: Vec<f64> = datasets
        .iter()
        .flat_map(|set| set.data.iter().copied())
        .filter(|v| v.is_finite())
        .collect();

    let raw_max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let raw_min = values.iter().copied().fold(f64::INFINITY, f64::min);

    if !raw_max.is_finite() || !raw_min.is_finite() {
        return Scale { min: 0.0, max: 1.0 };
    }

    let max = if raw_max > 0.0 { raw_max } else { 0.0 };
    let min = if raw_min < 0.0 { raw_min } else { 0.0 };

    if (max - min).abs() < f64::EPSILON {
        return Scale { min: 0.0, max: 1.0 };
    }

    let step = nice_step((max - min) / TICK_COUNT as f64);
    Scale {
        min: (min / step).floor() * step,
        max: (max / step).ceil() * step,
    }
}

/// Round a raw interval up to the nearest 1, 2, 2.5, 5, or 10 times a power of
/// ten — the intervals a reader can divide in their head.
fn nice_step(rough: f64) -> f64 {
    if rough <= 0.0 || !rough.is_finite() {
        return 1.0;
    }
    let magnitude = 10f64.powf(rough.log10().floor());
    let normalized = rough / magnitude;
    let stepped = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 2.5 {
        2.5
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    stepped * magnitude
}

pub(super) fn format_value(value: f64) -> String {
    if !value.is_finite() {
        return String::from("—");
    }
    if value.abs() >= 10_000.0 {
        return format!("{:.0}", value);
    }
    if (value.fract()).abs() < f64::EPSILON {
        return format!("{:.0}", value);
    }
    let rendered = format!("{:.2}", value);
    rendered
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

// Why: SVG coordinates carry no units, so a stable short form keeps the
// emitted markup diffable and free of float noise like `12.000000000000002`.
pub(super) fn coord(value: f64) -> String {
    format!("{:.2}", value)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}
