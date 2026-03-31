use std::io::Write;

use crate::models::{ColumnInfo, DatabaseInfo, QueryResult, TableInfo};

pub trait DatabaseCliDisplay {
    fn display_with_cli(&self);
}

fn stdout_writeln(args: std::fmt::Arguments<'_>) {
    let mut stdout = std::io::stdout();
    let _ = writeln!(stdout, "{args}");
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
                        || "NULL".to_string(),
                        |v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Null => "NULL".to_string(),
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
