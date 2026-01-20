pub fn parse_cron_human(schedule: &str) -> String {
    let parts: Vec<&str> = schedule.split_whitespace().collect();
    if parts.len() != 6 {
        return schedule.to_string();
    }

    match (parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]) {
        ("0", "0", "*", "*", "*", "*") => "Every hour".to_string(),
        ("0", min, "*", "*", "*", "*") if min.starts_with("*/") => {
            format!("Every {} minutes", &min[2..])
        },
        ("0", "0", hour, "*", "*", "*") if hour.starts_with("*/") => {
            format!("Every {} hours", &hour[2..])
        },
        ("0", "0", hour, "*", "*", "*") => format!("Daily at {}:00", hour),
        ("0", min, hour, "*", "*", "*") => format!("Daily at {}:{}", hour, min),
        ("*", "*", "*", "*", "*", "*") => "Every second".to_string(),
        _ => schedule.to_string(),
    }
}
