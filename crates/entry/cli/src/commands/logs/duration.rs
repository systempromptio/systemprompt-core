use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};

pub fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim().to_lowercase();

    if let Some(days) = s.strip_suffix('d') {
        let num: i64 = days
            .parse()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        return Ok(Duration::days(num));
    }

    if let Some(hours) = s.strip_suffix('h') {
        let num: i64 = hours
            .parse()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        return Ok(Duration::hours(num));
    }

    if let Some(mins) = s.strip_suffix('m') {
        let num: i64 = mins
            .parse()
            .map_err(|_| anyhow!("Invalid duration: {}", s))?;
        return Ok(Duration::minutes(num));
    }

    if let Ok(num) = s.parse::<i64>() {
        return Ok(Duration::days(num));
    }

    Err(anyhow!(
        "Invalid duration format: {}. Use formats like '7d', '24h', '30m'",
        s
    ))
}

pub fn parse_since(since: &Option<String>) -> Result<Option<DateTime<Utc>>> {
    let Some(s) = since else {
        return Ok(None);
    };

    let s = s.trim().to_lowercase();

    if let Ok(duration) = parse_duration(&s) {
        return Ok(Some(Utc::now() - duration));
    }

    if let Ok(date) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        let datetime = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Some(DateTime::from_naive_utc_and_offset(datetime, Utc)));
    }

    if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(Some(DateTime::from_naive_utc_and_offset(datetime, Utc)));
    }

    Err(anyhow!(
        "Invalid --since format: {}. Use formats like '1h', '24h', '7d', '2026-01-13', or '2026-01-13T10:00:00'",
        s
    ))
}
