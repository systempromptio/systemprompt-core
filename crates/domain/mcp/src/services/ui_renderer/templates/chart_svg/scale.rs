//! Axis range selection and number formatting for SVG charts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::artifacts::chart::ChartDataset;

pub(super) const TICK_COUNT: usize = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ScaleKind {
    #[default]
    Linear,
    Logarithmic,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Scale {
    pub min: f64,
    pub max: f64,
    pub kind: ScaleKind,
}

impl Scale {
    pub(super) fn fraction(self, value: f64) -> f64 {
        match self.kind {
            ScaleKind::Linear => {
                let span = self.max - self.min;
                if span <= 0.0 {
                    return 0.0;
                }
                ((value - self.min) / span).clamp(0.0, 1.0)
            },
            ScaleKind::Logarithmic => {
                // Why: A log axis cannot place zero or a negative, so anything at or
                // below the floor sits on it rather than vanishing.
                if value <= 0.0 || self.min <= 0.0 {
                    return 0.0;
                }
                let span = self.max.log10() - self.min.log10();
                if span <= 0.0 {
                    return 0.0;
                }
                ((value.log10() - self.min.log10()) / span).clamp(0.0, 1.0)
            },
        }
    }

    pub(super) fn ticks(self) -> Vec<f64> {
        match self.kind {
            ScaleKind::Linear => {
                let step = (self.max - self.min) / TICK_COUNT as f64;
                (0..=TICK_COUNT)
                    .map(|i| step.mul_add(i as f64, self.min))
                    .collect()
            },
            // Why: One tick per decade: the whole point of a log axis is that the
            // decades are evenly spaced.
            ScaleKind::Logarithmic => {
                let lo = self.min.log10().round() as i32;
                let hi = self.max.log10().round() as i32;
                (lo..=hi).map(|e| 10f64.powi(e)).collect()
            },
        }
    }
}

pub(super) fn for_axis(datasets: &[ChartDataset], kind: ScaleKind) -> Scale {
    match kind {
        ScaleKind::Linear => linear(datasets),
        ScaleKind::Logarithmic => logarithmic(datasets).unwrap_or_else(|| linear(datasets)),
    }
}

fn logarithmic(datasets: &[ChartDataset]) -> Option<Scale> {
    let positives: Vec<f64> = datasets
        .iter()
        .flat_map(|set| set.data.iter().copied())
        .filter(|v| v.is_finite() && *v > 0.0)
        .collect();

    let raw_max = positives.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let raw_min = positives.iter().copied().fold(f64::INFINITY, f64::min);
    if !raw_max.is_finite() || !raw_min.is_finite() {
        return None;
    }

    // Why: Snap out to whole decades so the ticks land on powers of ten.
    let min = 10f64.powf(raw_min.log10().floor());
    let max = 10f64.powf(raw_max.log10().ceil());
    let max = if (max - min).abs() < f64::EPSILON {
        min * 10.0
    } else {
        max
    };

    Some(Scale {
        min,
        max,
        kind: ScaleKind::Logarithmic,
    })
}

pub(super) fn linear(datasets: &[ChartDataset]) -> Scale {
    let values: Vec<f64> = datasets
        .iter()
        .flat_map(|set| set.data.iter().copied())
        .filter(|v| v.is_finite())
        .collect();

    let raw_max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let raw_min = values.iter().copied().fold(f64::INFINITY, f64::min);

    if !raw_max.is_finite() || !raw_min.is_finite() {
        return Scale {
            min: 0.0,
            max: 1.0,
            kind: ScaleKind::Linear,
        };
    }

    let max = if raw_max > 0.0 { raw_max } else { 0.0 };
    let min = if raw_min < 0.0 { raw_min } else { 0.0 };

    if (max - min).abs() < f64::EPSILON {
        return Scale {
            min: 0.0,
            max: 1.0,
            kind: ScaleKind::Linear,
        };
    }

    let step = nice_step((max - min) / TICK_COUNT as f64);
    Scale {
        min: (min / step).floor() * step,
        max: (max / step).ceil() * step,
        kind: ScaleKind::Linear,
    }
}

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

pub(super) fn coord(value: f64) -> String {
    format!("{:.2}", value)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}
