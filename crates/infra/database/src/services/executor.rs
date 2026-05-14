//! SQL batch and statement-by-statement execution helpers.

use super::database::Database;
use super::provider::DatabaseProvider;
use crate::error::{DatabaseResult, RepositoryError};
use crate::models::QueryResult;

#[derive(Debug, Copy, Clone)]
pub struct SqlExecutor;

enum SplitState {
    Normal,
    SingleQuote,
    DollarQuote(String),
    LineComment,
    BlockComment(u32),
}

fn dollar_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    debug_assert_eq!(bytes[start], b'$');
    let mut j = start + 1;
    while j < bytes.len() {
        let c = bytes[j];
        if c == b'$' {
            return Some(j);
        }
        if !(c.is_ascii_alphanumeric() || c == b'_') {
            return None;
        }
        j += 1;
    }
    None
}

impl SqlExecutor {
    pub async fn execute_statements(db: &Database, sql: &str) -> DatabaseResult<()> {
        db.execute_batch(sql).await.map_err(|e| {
            RepositoryError::Internal(format!("Failed to execute SQL statements: {e}"))
        })
    }

    pub async fn execute_statements_parsed(
        db: &dyn DatabaseProvider,
        sql: &str,
    ) -> DatabaseResult<()> {
        let statements = Self::parse_sql_statements(sql)?;

        for statement in statements {
            db.execute_raw(&statement).await.map_err(|e| {
                RepositoryError::Internal(format!(
                    "Failed to execute SQL statement: {statement}: {e}"
                ))
            })?;
        }

        Ok(())
    }

    /// Split a Postgres SQL script into individual statements while preserving
    /// the original source text. Splits on top-level `;`; ignores
    /// semicolons inside single quotes, dollar-quoted bodies (`$$ … $$` and
    /// `$tag$ … $tag$`), `--` line comments, and `/* … */` block comments
    /// (nested). Unterminated quotes or comments return
    /// `RepositoryError::Internal`; grammar errors are left for Postgres to
    /// surface at execute time. Preserving the original text is the
    /// reason this is hand-rolled rather than `sqlparser`: round-tripping
    /// through `Statement::Display` drops syntactic detail such as the
    /// empty parameter list on `CREATE FUNCTION foo()`, which Postgres then
    /// rejects.
    pub fn parse_sql_statements(sql: &str) -> DatabaseResult<Vec<String>> {
        let bytes = sql.as_bytes();
        let mut statements = Vec::new();
        let mut start = 0usize;
        let mut i = 0usize;
        let mut state = SplitState::Normal;
        let mut has_content = false;
        let mut emit = |sql: &str, start: usize, end: usize, has_content: &mut bool| {
            if *has_content {
                let stmt = sql[start..end].trim();
                if !stmt.is_empty() {
                    statements.push(stmt.to_string());
                }
            }
            *has_content = false;
        };

        while i < bytes.len() {
            match &mut state {
                SplitState::Normal => match bytes[i] {
                    b'\'' => {
                        has_content = true;
                        state = SplitState::SingleQuote;
                        i += 1;
                    },
                    b'-' if bytes.get(i + 1) == Some(&b'-') => {
                        state = SplitState::LineComment;
                        i += 2;
                    },
                    b'/' if bytes.get(i + 1) == Some(&b'*') => {
                        state = SplitState::BlockComment(1);
                        i += 2;
                    },
                    b'$' => {
                        has_content = true;
                        if let Some(tag_end) = dollar_tag_end(bytes, i) {
                            let tag = sql[i..=tag_end].to_string();
                            state = SplitState::DollarQuote(tag);
                            i = tag_end + 1;
                        } else {
                            i += 1;
                        }
                    },
                    b';' => {
                        emit(sql, start, i, &mut has_content);
                        i += 1;
                        start = i;
                    },
                    b => {
                        if !b.is_ascii_whitespace() {
                            has_content = true;
                        }
                        i += 1;
                    },
                },
                SplitState::SingleQuote => {
                    if bytes[i] == b'\'' {
                        if bytes.get(i + 1) == Some(&b'\'') {
                            i += 2;
                        } else {
                            state = SplitState::Normal;
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                },
                SplitState::DollarQuote(tag) => {
                    let tag_bytes = tag.as_bytes();
                    if i + tag_bytes.len() <= bytes.len()
                        && &bytes[i..i + tag_bytes.len()] == tag_bytes
                    {
                        i += tag_bytes.len();
                        state = SplitState::Normal;
                    } else {
                        i += 1;
                    }
                },
                SplitState::LineComment => {
                    if bytes[i] == b'\n' {
                        state = SplitState::Normal;
                    }
                    i += 1;
                },
                SplitState::BlockComment(depth) => {
                    if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        *depth += 1;
                        i += 2;
                    } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        *depth -= 1;
                        i += 2;
                        if *depth == 0 {
                            state = SplitState::Normal;
                        }
                    } else {
                        i += 1;
                    }
                },
            }
        }

        match state {
            SplitState::Normal | SplitState::LineComment => {
                emit(sql, start, sql.len(), &mut has_content);
                Ok(statements)
            },
            SplitState::SingleQuote => Err(RepositoryError::Internal(
                "Unterminated string literal in SQL".into(),
            )),
            SplitState::DollarQuote(tag) => Err(RepositoryError::Internal(format!(
                "Unterminated dollar-quoted string: {tag}"
            ))),
            SplitState::BlockComment(_) => Err(RepositoryError::Internal(
                "Unterminated block comment in SQL".into(),
            )),
        }
    }

    pub async fn execute_query(db: &Database, query: &str) -> DatabaseResult<QueryResult> {
        db.query(&query)
            .await
            .map_err(|e| RepositoryError::Internal(format!("Failed to execute query: {e}")))
    }

    pub async fn execute_file(db: &Database, file_path: &str) -> DatabaseResult<()> {
        let sql = std::fs::read_to_string(file_path).map_err(|e| {
            RepositoryError::Internal(format!("Failed to read SQL file: {file_path}: {e}"))
        })?;
        Self::execute_statements(db, &sql).await
    }

    pub async fn execute_file_parsed(
        db: &dyn DatabaseProvider,
        file_path: &str,
    ) -> DatabaseResult<()> {
        let sql = std::fs::read_to_string(file_path).map_err(|e| {
            RepositoryError::Internal(format!("Failed to read SQL file: {file_path}: {e}"))
        })?;
        Self::execute_statements_parsed(db, &sql).await
    }

    pub async fn table_exists(db: &Database, table_name: &str) -> DatabaseResult<bool> {
        let result = db
            .query_with(
                &"SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = \
                  'public' AND table_name = $1) as exists",
                vec![serde_json::Value::String(table_name.to_string())],
            )
            .await?;

        result
            .first()
            .and_then(|row| row.get("exists"))
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| RepositoryError::Internal("Failed to check table existence".to_string()))
    }

    pub async fn column_exists(
        db: &Database,
        table_name: &str,
        column_name: &str,
    ) -> DatabaseResult<bool> {
        let result = db
            .query_with(
                &"SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = \
                  'public' AND table_name = $1 AND column_name = $2) as exists",
                vec![
                    serde_json::Value::String(table_name.to_string()),
                    serde_json::Value::String(column_name.to_string()),
                ],
            )
            .await?;

        result
            .first()
            .and_then(|row| row.get("exists"))
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| {
                RepositoryError::Internal("Failed to check column existence".to_string())
            })
    }
}
