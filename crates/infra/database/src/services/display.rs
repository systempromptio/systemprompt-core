//! CLI display traits for printing query results, table descriptors, and
//! database info to stdout.
//!
//! A display write is best-effort: the command's outcome does not depend on
//! whether the terminal accepted the bytes, so a failed write is reported
//! through `tracing` via [`report_write_failure`] and the caller continues.
//! A closed downstream pipe is not reported at all — with SIGPIPE ignored
//! every later write would fail the same way and flood the log for a reader
//! that has already gone away. Every stdio display sink in the infra layer
//! routes its failures through that one helper.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Write;

use crate::models::{ColumnInfo, DatabaseInfo, QueryResult, TableInfo};

pub trait DatabaseCliDisplay {
    fn display_with_cli(&self);
}

pub fn report_write_failure(sink: &'static str, error: &std::io::Error) {
    if error.kind() == std::io::ErrorKind::BrokenPipe {
        return;
    }
    tracing::warn!(sink, error = %error, "Display sink write failed");
}

fn stdout_writeln(args: std::fmt::Arguments<'_>) {
    if let Err(error) = writeln!(std::io::stdout(), "{args}") {
        report_write_failure("stdout", &error);
    }
}

impl DatabaseCliDisplay for Vec<TableInfo> {
    fn display_with_cli(&self) {
        if self.is_empty() {
            stdout_writeln(format_args!("No tables found"));
        } else {
            stdout_writeln(format_args!("Tables:"));
            for table in self {
                stdout_writeln(format_args!("  {} (rows: {})", table.name, table.row_count));
            }
        }
    }
}

impl DatabaseCliDisplay for (Vec<ColumnInfo>, i64) {
    fn display_with_cli(&self) {
        let (columns, _) = self;
        stdout_writeln(format_args!("Columns:"));
        for col in columns {
            let default_display = col
                .default
                .as_deref()
                .map_or_else(String::new, |d| format!("DEFAULT {d}"));

            stdout_writeln(format_args!(
                "  {} {} {} {} {}",
                col.name,
                col.data_type,
                if col.nullable { "NULL" } else { "NOT NULL" },
                if col.primary_key { "PK" } else { "" },
                default_display
            ));
        }
    }
}

impl DatabaseCliDisplay for DatabaseInfo {
    fn display_with_cli(&self) {
        stdout_writeln(format_args!("Database Info:"));
        stdout_writeln(format_args!("  Path: {}", self.path));
        stdout_writeln(format_args!("  Version: {}", self.version));
        stdout_writeln(format_args!("  Tables: {}", self.tables.len()));
    }
}

impl DatabaseCliDisplay for QueryResult {
    fn display_with_cli(&self) {
        if self.columns.is_empty() {
            stdout_writeln(format_args!("No data returned"));
            return;
        }

        stdout_writeln(format_args!("{}", self.columns.join(" | ")));
        stdout_writeln(format_args!("{}", "-".repeat(80)));

        for row in &self.rows {
            let values: Vec<String> = self
                .columns
                .iter()
                .map(|col| {
                    row.get(col).map_or_else(
                        || "NULL".to_owned(),
                        |v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Null => "NULL".to_owned(),
                            serde_json::Value::Bool(_)
                            | serde_json::Value::Number(_)
                            | serde_json::Value::Array(_)
                            | serde_json::Value::Object(_) => v.to_string(),
                        },
                    )
                })
                .collect();
            stdout_writeln(format_args!("{}", values.join(" | ")));
        }

        stdout_writeln(format_args!(
            "\n{} rows returned in {}ms",
            self.row_count, self.execution_time_ms
        ));
    }
}
