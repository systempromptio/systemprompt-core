use serde::Serialize;

const KNOWN_TABLES: &[&str] = &[
    "logs",
    "ai_requests",
    "mcp_tool_executions",
    "agent_tasks",
    "users",
    "tenants",
    "sessions",
    "agent_execution_steps",
    "agent_artifacts",
    "credentials",
    "mcp_servers",
    "workflow_states",
    "blog_posts",
    "categories",
    "sources",
];

pub fn format_bytes(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

pub fn extract_relation_name(msg: &str) -> String {
    if let Some(start) = msg.find('"') {
        if let Some(end) = msg[start + 1..].find('"') {
            return msg[start + 1..start + 1 + end].to_string();
        }
    }
    "unknown".to_string()
}

pub fn suggest_table_name(input: &str) -> Option<String> {
    let input_lower = input.to_lowercase();
    let input_parts: Vec<&str> = input_lower.split('_').collect();

    KNOWN_TABLES
        .iter()
        .filter(|&&table| {
            let table_lower = table.to_lowercase();
            let table_parts: Vec<&str> = table_lower.split('_').collect();

            table_lower.contains(&input_lower)
                || input_lower.contains(&table_lower)
                || levenshtein_distance(&input_lower, &table_lower) <= 4
                || shares_prefix_parts(&input_parts, &table_parts, 2)
        })
        .min_by_key(|&&table| levenshtein_distance(&input_lower, &table.to_lowercase()))
        .map(|&s| s.to_string())
}

fn shares_prefix_parts(a: &[&str], b: &[&str], min_shared: usize) -> bool {
    let shared = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    shared >= min_shared
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut prev_row: Vec<usize> = (0..=len2).collect();
    let mut curr_row: Vec<usize> = vec![0; len2 + 1];

    for (i, c1) in s1.chars().enumerate() {
        curr_row[0] = i + 1;

        for (j, c2) in s2.chars().enumerate() {
            let cost = usize::from(c1 != c2);
            curr_row[j + 1] = (prev_row[j + 1] + 1)
                .min(curr_row[j] + 1)
                .min(prev_row[j] + cost);
        }

        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[len2]
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonError {
    pub error: bool,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_tables: Option<Vec<String>>,
}

impl JsonError {
    pub fn table_not_found(table: &str) -> Self {
        let suggestion = suggest_table_name(table);
        let hint = suggestion.map(|s| format!("Did you mean '{}'?", s));

        Self {
            error: true,
            code: "TABLE_NOT_FOUND".to_string(),
            message: format!("Table '{}' not found", table),
            hint,
            available_tables: Some(KNOWN_TABLES.iter().map(|s| (*s).to_string()).collect()),
        }
    }
}
