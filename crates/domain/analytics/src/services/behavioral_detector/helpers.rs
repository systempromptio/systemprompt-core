//! Shared pure helpers used by both `checks` (single-session) and
//! `fingerprint_checks` (cross-session) detector modules.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};

use super::thresholds;

pub(super) fn is_sequential_crawl(endpoints: &[String]) -> bool {
    if endpoints.len() < 5 {
        return false;
    }

    let mut sorted = endpoints.to_vec();
    sorted.sort();

    let matches = endpoints
        .iter()
        .enumerate()
        .filter(|(i, endpoint)| *i < sorted.len() && *endpoint == &sorted[*i])
        .count();

    (matches as f64 / endpoints.len() as f64) > 0.7
}

pub(super) fn compute_timing_variance(timestamps: &[DateTime<Utc>]) -> Option<f64> {
    if timestamps.len() < 2 {
        return None;
    }

    let intervals: Vec<f64> = timestamps
        .windows(2)
        .map(|w| (w[1] - w[0]).num_milliseconds() as f64)
        .collect();

    if intervals.is_empty() {
        return None;
    }

    let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
    if mean <= 0.0 {
        return None;
    }

    let variance =
        intervals.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / intervals.len() as f64;
    let std_dev = variance.sqrt();

    Some(std_dev / mean)
}

pub(super) fn is_outdated_browser(user_agent: &str) -> bool {
    let ua_lower = user_agent.to_lowercase();

    if let Some(pos) = ua_lower.find("chrome/")
        && let Some(dot_pos) = ua_lower[pos + 7..].find('.')
        && let Ok(major) = ua_lower[pos + 7..][..dot_pos].parse::<i32>()
        && major < thresholds::CHROME_MIN_VERSION
    {
        return true;
    }

    if let Some(pos) = ua_lower.find("firefox/") {
        let version_str = &ua_lower[pos + 8..];
        if let Some(space_pos) = version_str.find(|c: char| !c.is_numeric() && c != '.')
            && let Ok(major) = version_str[..space_pos].parse::<i32>()
            && major < thresholds::FIREFOX_MIN_VERSION
        {
            return true;
        } else if let Ok(major) = version_str.parse::<i32>()
            && major < thresholds::FIREFOX_MIN_VERSION
        {
            return true;
        }
    }

    false
}
