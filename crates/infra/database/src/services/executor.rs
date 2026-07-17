//! SQL batch and statement-by-statement execution helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

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

struct Splitter<'a> {
    sql: &'a str,
    bytes: &'a [u8],
    i: usize,
    start: usize,
    has_content: bool,
    statements: Vec<String>,
}

impl<'a> Splitter<'a> {
    const fn new(sql: &'a str) -> Self {
        Self {
            sql,
            bytes: sql.as_bytes(),
            i: 0,
            start: 0,
            has_content: false,
            statements: Vec::new(),
        }
    }

    fn emit(&mut self, end: usize) {
        if self.has_content {
            let stmt = self.sql[self.start..end].trim();
            if !stmt.is_empty() {
                self.statements.push(stmt.to_owned());
            }
        }
        self.has_content = false;
    }

    fn step_normal(&mut self) -> SplitState {
        match self.bytes[self.i] {
            b'\'' => {
                self.has_content = true;
                self.i += 1;
                SplitState::SingleQuote
            },
            b'-' if self.bytes.get(self.i + 1) == Some(&b'-') => {
                self.i += 2;
                SplitState::LineComment
            },
            b'/' if self.bytes.get(self.i + 1) == Some(&b'*') => {
                self.i += 2;
                SplitState::BlockComment(1)
            },
            b'$' => {
                self.has_content = true;
                if let Some(tag_end) = dollar_tag_end(self.bytes, self.i) {
                    let tag = self.sql[self.i..=tag_end].to_string();
                    self.i = tag_end + 1;
                    SplitState::DollarQuote(tag)
                } else {
                    self.i += 1;
                    SplitState::Normal
                }
            },
            b';' => {
                self.emit(self.i);
                self.i += 1;
                self.start = self.i;
                SplitState::Normal
            },
            b => {
                if !b.is_ascii_whitespace() {
                    self.has_content = true;
                }
                self.i += 1;
                SplitState::Normal
            },
        }
    }

    fn step_single_quote(&mut self) -> SplitState {
        if self.bytes[self.i] == b'\'' {
            if self.bytes.get(self.i + 1) == Some(&b'\'') {
                self.i += 2;
                SplitState::SingleQuote
            } else {
                self.i += 1;
                SplitState::Normal
            }
        } else {
            self.i += 1;
            SplitState::SingleQuote
        }
    }

    fn step_dollar_quote(&mut self, tag: String) -> SplitState {
        let tag_bytes = tag.as_bytes();
        if self.i + tag_bytes.len() <= self.bytes.len()
            && self.bytes[self.i..self.i + tag_bytes.len()] == *tag_bytes
        {
            self.i += tag_bytes.len();
            SplitState::Normal
        } else {
            self.i += 1;
            SplitState::DollarQuote(tag)
        }
    }

    const fn step_line_comment(&mut self) -> SplitState {
        let next = if self.bytes[self.i] == b'\n' {
            SplitState::Normal
        } else {
            SplitState::LineComment
        };
        self.i += 1;
        next
    }

    fn step_block_comment(&mut self, depth: u32) -> SplitState {
        if self.bytes[self.i] == b'/' && self.bytes.get(self.i + 1) == Some(&b'*') {
            self.i += 2;
            SplitState::BlockComment(depth + 1)
        } else if self.bytes[self.i] == b'*' && self.bytes.get(self.i + 1) == Some(&b'/') {
            self.i += 2;
            if depth == 1 {
                SplitState::Normal
            } else {
                SplitState::BlockComment(depth - 1)
            }
        } else {
            self.i += 1;
            SplitState::BlockComment(depth)
        }
    }

    fn run(mut self) -> DatabaseResult<Vec<String>> {
        let mut state = SplitState::Normal;
        while self.i < self.bytes.len() {
            state = match state {
                SplitState::Normal => self.step_normal(),
                SplitState::SingleQuote => self.step_single_quote(),
                SplitState::DollarQuote(tag) => self.step_dollar_quote(tag),
                SplitState::LineComment => self.step_line_comment(),
                SplitState::BlockComment(depth) => self.step_block_comment(depth),
            };
        }

        match state {
            SplitState::Normal | SplitState::LineComment => {
                let end = self.sql.len();
                self.emit(end);
                Ok(self.statements)
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

    pub fn parse_sql_statements(sql: &str) -> DatabaseResult<Vec<String>> {
        Splitter::new(sql).run()
    }

    pub async fn execute_query(db: &Database, query: &str) -> DatabaseResult<QueryResult> {
        db.query_raw(&query)
            .await
            .map_err(|e| RepositoryError::QueryExecution(Box::new(e)))
    }

    pub async fn execute_file(db: &Database, file_path: &str) -> DatabaseResult<()> {
        let sql = std::fs::read_to_string(file_path).map_err(|e| {
            RepositoryError::Internal(format!("Failed to read SQL file: {file_path}: {e}"))
        })?;
        Self::execute_statements(db, &sql).await
    }

    pub async fn table_exists(db: &Database, table_name: &str) -> DatabaseResult<bool> {
        let result = db
            .query_raw_with(
                &"SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = \
                  'public' AND table_name = $1) as exists",
                &[&table_name],
            )
            .await?;

        result
            .first()
            .and_then(|row| row.get("exists"))
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| RepositoryError::Internal("Failed to check table existence".to_owned()))
    }

    pub async fn column_exists(
        db: &Database,
        table_name: &str,
        column_name: &str,
    ) -> DatabaseResult<bool> {
        let result = db
            .query_raw_with(
                &"SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = \
                  'public' AND table_name = $1 AND column_name = $2) as exists",
                &[&table_name, &column_name],
            )
            .await?;

        result
            .first()
            .and_then(|row| row.get("exists"))
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| RepositoryError::Internal("Failed to check column existence".to_owned()))
    }
}
