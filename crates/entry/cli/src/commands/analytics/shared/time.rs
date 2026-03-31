use anyhow::{Result, anyhow};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Timelike, Utc};

pub fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim().to_lowercase();

    s.strip_suffix('d')
        .and_then(|d| d.parse::<i64>().ok())
        .map(Duration::days)
        .or_else(|| {
            s.strip_suffix('h')
                .and_then(|h| h.parse::<i64>().ok())
                .map(Duration::hours)
        })
        .or_else(|| {
            s.strip_suffix('m')
                .and_then(|m| m.parse::<i64>().ok())
                .map(Duration::minutes)
        })
        .or_else(|| {
            s.strip_suffix('w')
                .and_then(|w| w.parse::<i64>().ok())
                .map(Duration::weeks)
        })
}

pub fn parse_since(since: Option<&String>) -> Result<Option<DateTime<Utc>>> {
    let Some(s) = since else {
        return Ok(None);
    };

    let s = s.trim().to_lowercase();

    if let Some(duration) = parse_duration(&s) {
        return Ok(Some(Utc::now() - duration));
    }

    if let Ok(date) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        return date
            .and_hms_opt(0, 0, 0)
            .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc))
            .map(Some)
            .ok_or_else(|| anyhow!("Invalid date: {}", s));
    }

    chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S")
        .map(|naive| Some(DateTime::from_naive_utc_and_offset(naive, Utc)))
        .map_err(|_| {
            anyhow!(
                "Invalid --since format: {}. Use '1h', '24h', '7d', '2026-01-13', or \
                 '2026-01-13T10:00:00'",
                s
            )
        })
}

pub fn parse_until(until: Option<&String>) -> Result<Option<DateTime<Utc>>> {
    let Some(s) = until else {
        return Ok(None);
    };

    let s = s.trim().to_lowercase();

    if let Some(duration) = parse_duration(&s) {
        return Ok(Some(Utc::now() - duration));
    }

    if let Ok(date) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        return date
            .and_hms_opt(23, 59, 59)
            .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc))
            .map(Some)
            .ok_or_else(|| anyhow!("Invalid date: {}", s));
    }

    chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S")
        .map(|naive| Some(DateTime::from_naive_utc_and_offset(naive, Utc)))
        .map_err(|_| {
            anyhow!(
                "Invalid --until format: {}. Use '1h', '24h', '7d', '2026-01-13', or \
                 '2026-01-13T10:00:00'",
                s
            )
        })
}

pub fn parse_time_range(
    since: Option<&String>,
    until: Option<&String>,
) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let start = parse_since(since)?.unwrap_or_else(|| Utc::now() - Duration::hours(24));
    let end = parse_until(until)?.unwrap_or_else(Utc::now);
    Ok((start, end))
}

pub fn truncate_to_period(dt: DateTime<Utc>, period: &str) -> DateTime<Utc> {
    match period {
        "hour" => dt
            .date_naive()
            .and_hms_opt(dt.time().hour(), 0, 0)
            .map_or(dt, |naive| DateTime::from_naive_utc_and_offset(naive, Utc)),
        "day" => dt
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map_or(dt, |naive| DateTime::from_naive_utc_and_offset(naive, Utc)),
        "week" => {
            let days_since_monday = dt.weekday().num_days_from_monday();
            (dt.date_naive() - Duration::days(i64::from(days_since_monday)))
                .and_hms_opt(0, 0, 0)
                .map_or(dt, |naive| DateTime::from_naive_utc_and_offset(naive, Utc))
        },
        "month" => dt
            .date_naive()
            .with_day(1)
            .and_then(|d: NaiveDate| d.and_hms_opt(0, 0, 0))
            .map_or(dt, |naive| DateTime::from_naive_utc_and_offset(naive, Utc)),
        _ => dt,
    }
}

pub use systemprompt_models::time_format::{format_duration_ms, format_period_label, format_timestamp};
