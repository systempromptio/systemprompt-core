//! Axis range selection for SVG charts.
//!
//! A chart that renders is not a chart that is right. These are the decisions
//! that place a point on the canvas: get the range wrong and every series is
//! drawn confidently at the wrong height, with nothing to show for it.

use systemprompt_mcp::test_api::{Scale, ScaleKind, for_axis, format_value, linear};
use systemprompt_models::artifacts::chart::ChartDataset;

fn data(values: &[f64]) -> Vec<ChartDataset> {
    vec![ChartDataset::new("series", values.to_vec())]
}

// Why: a linear axis includes zero even when the data does not go near it.
// Starting the axis at the lowest data point exaggerates small differences
// into dramatic ones — the classic misleading chart.
#[test]
fn a_linear_axis_over_positive_data_includes_zero() {
    let scale = linear(&data(&[100.0, 105.0, 110.0]));

    assert_eq!(
        scale.min, 0.0,
        "a positive series must be measured from zero, not from its own floor"
    );
    assert!(scale.max >= 110.0, "the axis must contain the data");
}

#[test]
fn a_linear_axis_over_negative_data_extends_below_zero() {
    let scale = linear(&data(&[-40.0, -10.0]));

    assert!(scale.min <= -40.0, "the axis must contain the lowest point");
    assert_eq!(scale.max, 0.0, "a wholly negative series tops out at zero");
}

// Why: with no finite values there is no range to compute. A NaN or infinite
// bound would propagate into every coordinate and render nothing at all, so
// the fallback is a unit axis.
#[test]
fn a_series_with_no_finite_values_falls_back_to_a_unit_axis() {
    for values in [
        vec![],
        vec![f64::NAN],
        vec![f64::INFINITY, f64::NEG_INFINITY],
    ] {
        let scale = linear(&data(&values));
        assert_eq!(
            (scale.min, scale.max),
            (0.0, 1.0),
            "no finite data means no range: {values:?}"
        );
    }
}

// Why: non-finite values are dropped rather than poisoning the bounds. One NaN
// in a series would otherwise make max NaN and lose the whole chart.
#[test]
fn non_finite_values_are_ignored_rather_than_poisoning_the_range() {
    let scale = linear(&data(&[10.0, f64::NAN, 50.0, f64::INFINITY]));

    assert!(scale.max.is_finite(), "one NaN must not lose the axis");
    assert!(scale.max >= 50.0, "the finite data must still fit");
}

// Why: an all-zero series is the one case with genuinely no span — zero floor,
// zero ceiling. Dividing by that span would put every point at the same
// coordinate or produce NaN, so it degenerates to a unit axis.
#[test]
fn an_all_zero_series_degenerates_to_a_unit_axis() {
    let scale = linear(&data(&[0.0, 0.0]));

    assert_eq!((scale.min, scale.max), (0.0, 1.0));
}

// Why: a flat *positive* series is not degenerate, because the axis is
// measured from zero — the span is the value itself. It gets a real axis
// rounded out to a nice step, and the series draws as a flat line partway up
// rather than collapsing onto the floor.
#[test]
fn a_flat_positive_series_still_gets_a_real_axis_from_zero() {
    let scale = linear(&data(&[7.0, 7.0, 7.0]));

    assert_eq!(scale.min, 0.0);
    assert_eq!(
        scale.max, 8.0,
        "7 rounds up to the next step of 2, so the line sits below the top"
    );
    assert!(
        scale.fraction(7.0) > 0.5 && scale.fraction(7.0) < 1.0,
        "the flat line must be visible inside the plot, not on its edge"
    );
}

// Why: `fraction` is what turns a value into a position. The endpoints must
// land exactly on the ends, or every chart is drawn slightly off its frame.
#[test]
fn fraction_maps_the_axis_ends_to_zero_and_one() {
    let scale = Scale {
        min: 0.0,
        max: 200.0,
        kind: ScaleKind::Linear,
    };

    assert!((scale.fraction(0.0) - 0.0).abs() < 1e-9);
    assert!((scale.fraction(200.0) - 1.0).abs() < 1e-9);
    assert!((scale.fraction(100.0) - 0.5).abs() < 1e-9);
}

// Why: a value outside the axis must be clamped, not extrapolated. An
// unclamped fraction draws the point outside the plot area, over the axes and
// labels.
#[test]
fn fraction_clamps_values_outside_the_axis() {
    let scale = Scale {
        min: 0.0,
        max: 10.0,
        kind: ScaleKind::Linear,
    };

    assert!((scale.fraction(-5.0) - 0.0).abs() < 1e-9);
    assert!((scale.fraction(999.0) - 1.0).abs() < 1e-9);
}

// Why: a log axis has no position for zero or a negative. They sit on the
// floor rather than producing -inf and vanishing from the chart.
#[test]
fn a_log_axis_places_zero_and_negatives_on_the_floor() {
    let scale = Scale {
        min: 1.0,
        max: 1000.0,
        kind: ScaleKind::Logarithmic,
    };

    assert!((scale.fraction(0.0) - 0.0).abs() < 1e-9);
    assert!((scale.fraction(-10.0) - 0.0).abs() < 1e-9);
}

// Why: the point of a log axis is evenly spaced decades. A decade must be the
// same distance wherever it falls on the axis.
#[test]
fn a_log_axis_spaces_decades_evenly() {
    let scale = Scale {
        min: 1.0,
        max: 1000.0,
        kind: ScaleKind::Logarithmic,
    };

    let first = scale.fraction(10.0);
    let second = scale.fraction(100.0);

    assert!((first - 1.0 / 3.0).abs() < 1e-9, "one decade of three");
    assert!((second - 2.0 / 3.0).abs() < 1e-9, "two decades of three");
}

#[test]
fn log_ticks_are_powers_of_ten_across_the_range() {
    let scale = Scale {
        min: 1.0,
        max: 1000.0,
        kind: ScaleKind::Logarithmic,
    };

    assert_eq!(scale.ticks(), vec![1.0, 10.0, 100.0, 1000.0]);
}

#[test]
fn linear_ticks_span_the_axis_end_to_end() {
    let scale = Scale {
        min: 0.0,
        max: 100.0,
        kind: ScaleKind::Linear,
    };

    let ticks = scale.ticks();
    assert_eq!(ticks.first().copied(), Some(0.0));
    assert_eq!(ticks.last().copied(), Some(100.0));
}

// Why: a log axis needs a positive value to take a logarithm of. Asking for
// one over data that has none must fall back to linear rather than producing
// a NaN axis.
#[test]
fn a_log_axis_over_non_positive_data_falls_back_to_linear() {
    let scale = for_axis(&data(&[0.0, -5.0]), ScaleKind::Logarithmic);

    assert_eq!(
        scale.kind,
        ScaleKind::Linear,
        "with nothing positive to plot, a log axis is not available"
    );
    assert!(scale.min.is_finite() && scale.max.is_finite());
}

// Why: a log axis snaps out to whole decades so the ticks land on powers of
// ten rather than at arbitrary data values.
#[test]
fn a_log_axis_snaps_out_to_whole_decades() {
    let scale = for_axis(&data(&[3.0, 400.0]), ScaleKind::Logarithmic);

    assert_eq!(scale.kind, ScaleKind::Logarithmic);
    assert!(
        (scale.min - 1.0).abs() < 1e-9,
        "3 floors to the 10^0 decade"
    );
    assert!(
        (scale.max - 1000.0).abs() < 1e-9,
        "400 ceilings to the 10^3 decade"
    );
}

// Why: labels are read, not measured. Trailing zeros and a bare decimal point
// are noise on an axis, and a non-finite value must not render as "NaN".
#[test]
fn axis_labels_drop_trailing_zeros_and_never_render_nan() {
    assert_eq!(format_value(5.0), "5");
    assert_eq!(format_value(5.5), "5.5");
    assert_eq!(format_value(5.50), "5.5");
    assert_eq!(
        format_value(12_345.6),
        "12346",
        "large values lose the decimal"
    );
    assert_eq!(format_value(f64::NAN), "—");
    assert_eq!(format_value(f64::INFINITY), "—");
}
