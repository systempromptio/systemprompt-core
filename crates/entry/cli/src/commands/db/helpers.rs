use serde::Serialize;
use systemprompt_core_logging::CliService;

use crate::cli_settings::CliConfig;

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

#[derive(Debug, Clone, Serialize)]
pub struct JsonError {
    pub error: bool,
    pub code: String,
    pub message: String,
}

impl JsonError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            error: true,
            code: code.to_string(),
            message: message.to_string(),
        }
    }

    pub fn table_not_found(table: &str) -> Self {
        Self::new("TABLE_NOT_FOUND", &format!("Table '{}' not found", table))
    }

    pub fn query_failed(msg: &str) -> Self {
        Self::new("QUERY_FAILED", msg)
    }

    pub fn connection_failed() -> Self {
        Self::new(
            "CONNECTION_FAILED",
            "Failed to connect to database. Check your profile configuration.",
        )
    }

    pub fn user_not_found(user: &str) -> Self {
        Self::new("USER_NOT_FOUND", &format!("User '{}' not found", user))
    }
}

pub fn output_error(config: &CliConfig, error: &JsonError) {
    if config.is_json_output() {
        CliService::json(error);
    } else {
        CliService::error(&error.message);
    }
}
