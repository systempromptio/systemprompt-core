use chrono::{DateTime, Utc};

pub fn format_timestamp(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn format_date(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d").to_string()
}

pub fn format_date_range(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    format!("{} to {}", format_date(start), format_date(end))
}

pub fn format_duration_ms(ms: i64) -> String {
    match ms {
        ms if ms < 1000 => format!("{}ms", ms),
        ms if ms < 60_000 => format!("{:.1}s", ms as f64 / 1000.0),
        ms if ms < 3_600_000 => format!("{:.1}m", ms as f64 / 60_000.0),
        _ => format!("{:.1}h", ms as f64 / 3_600_000.0),
    }
}

pub fn format_optional_duration_ms(ms: Option<i64>) -> String {
    ms.map_or_else(String::new, |d| format!(" ({}ms)", d))
}

pub fn format_period_label(dt: DateTime<Utc>, period: &str) -> String {
    match period {
        "hour" => dt.format("%Y-%m-%d %H:00").to_string(),
        "day" => format_date(dt),
        "week" => format!("Week of {}", format_date(dt)),
        "month" => dt.format("%Y-%m").to_string(),
        _ => format_timestamp(dt),
    }
}
