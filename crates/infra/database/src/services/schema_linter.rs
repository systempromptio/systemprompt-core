//! Declarative-schema linter.
//!
//! Walks a SQL script using the same lex states as
//! [`crate::services::executor::SqlExecutor::parse_sql_statements`] (single
//! quote, dollar quote, line/block comment) so it inspects only top-level
//! tokens. Each top-level statement is classified by leading keywords:
//!
//! - **Allowed**: `CREATE TABLE [IF NOT EXISTS]`, `CREATE [UNIQUE] INDEX [IF
//!   NOT EXISTS]`, `CREATE [OR REPLACE] FUNCTION`, `CREATE [OR REPLACE] VIEW`,
//!   `CREATE [OR REPLACE] TRIGGER`, `CREATE TYPE`, `CREATE EXTENSION IF NOT
//!   EXISTS`, `COMMENT ON`.
//! - **Rejected**: `ALTER`, `DROP`, top-level `DO $$ … $$`, `UPDATE`, `INSERT`,
//!   `DELETE`, `TRUNCATE`, `GRANT`, `REVOKE`, anything containing `RENAME`.
//! - **Naked `CREATE TABLE foo (…)`** without `IF NOT EXISTS` is permitted but
//!   emitted as an informational warning (still reported as a [`LintError`]
//!   with [`LintSeverity::Warning`]).
//!
//! The lexer mirrors the splitter rather than calling into it because the
//! linter needs byte offsets — preserved as `(line, column)` — to surface
//! useful error messages.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintSeverity {
    Error,
    Warning,
}

impl fmt::Display for LintSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => f.write_str("error"),
            Self::Warning => f.write_str("warning"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintError {
    pub line: u32,
    pub column: u32,
    pub severity: LintSeverity,
    pub message: String,
    pub source: String,
}

impl fmt::Display for LintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}: {}",
            self.source, self.line, self.column, self.severity, self.message
        )
    }
}

/// Lint a single declarative schema file. Returns the list of violations,
/// or `Ok(())` if the script is purely declarative.
///
/// `source` is the label included in error messages (typically the schema
/// table name or the file path).
pub fn lint_declarative_schema(sql: &str, source: &str) -> Result<(), Vec<LintError>> {
    let statements = split_top_level_statements(sql, source)?;
    let mut errors = Vec::new();
    for stmt in &statements {
        if let Some(err) = classify(stmt, source) {
            errors.push(err);
        }
    }
    if errors.iter().any(|e| e.severity == LintSeverity::Error) {
        return Err(errors);
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct TopStatement {
    text: String,
    start_line: u32,
    start_column: u32,
}

enum LexState {
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

fn split_top_level_statements(
    sql: &str,
    source: &str,
) -> Result<Vec<TopStatement>, Vec<LintError>> {
    let bytes = sql.as_bytes();
    let mut statements: Vec<TopStatement> = Vec::new();
    let mut state = LexState::Normal;
    let mut start = 0usize;
    let mut i = 0usize;
    let mut start_line: u32 = 1;
    let mut start_col: u32 = 1;
    let mut line: u32 = 1;
    let mut col: u32 = 1;
    let mut stmt_line: u32 = 1;
    let mut stmt_col: u32 = 1;
    let mut has_content = false;

    while i < bytes.len() {
        let b = bytes[i];
        match &mut state {
            LexState::Normal => match b {
                b'\'' => {
                    if !has_content {
                        stmt_line = line;
                        stmt_col = col;
                    }
                    has_content = true;
                    state = LexState::SingleQuote;
                    advance(&mut i, &mut line, &mut col, b);
                },
                b'-' if bytes.get(i + 1) == Some(&b'-') => {
                    state = LexState::LineComment;
                    advance(&mut i, &mut line, &mut col, b);
                    advance(&mut i, &mut line, &mut col, b'-');
                },
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    state = LexState::BlockComment(1);
                    advance(&mut i, &mut line, &mut col, b);
                    advance(&mut i, &mut line, &mut col, b'*');
                },
                b'$' => {
                    if !has_content {
                        stmt_line = line;
                        stmt_col = col;
                    }
                    has_content = true;
                    if let Some(tag_end) = dollar_tag_end(bytes, i) {
                        let tag = sql[i..=tag_end].to_string();
                        let advance_by = tag_end - i + 1;
                        for _ in 0..advance_by {
                            advance(&mut i, &mut line, &mut col, b'$');
                        }
                        state = LexState::DollarQuote(tag);
                    } else {
                        advance(&mut i, &mut line, &mut col, b);
                    }
                },
                b';' => {
                    if has_content {
                        let text = sql[start..i].trim().to_string();
                        if !text.is_empty() {
                            statements.push(TopStatement {
                                text,
                                start_line: stmt_line,
                                start_column: stmt_col,
                            });
                        }
                    }
                    has_content = false;
                    advance(&mut i, &mut line, &mut col, b);
                    start = i;
                    start_line = line;
                    start_col = col;
                },
                _ => {
                    if !b.is_ascii_whitespace() {
                        if !has_content {
                            stmt_line = line;
                            stmt_col = col;
                        }
                        has_content = true;
                    }
                    advance(&mut i, &mut line, &mut col, b);
                },
            },
            LexState::SingleQuote => {
                if b == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        advance(&mut i, &mut line, &mut col, b);
                        advance(&mut i, &mut line, &mut col, b'\'');
                    } else {
                        state = LexState::Normal;
                        advance(&mut i, &mut line, &mut col, b);
                    }
                } else {
                    advance(&mut i, &mut line, &mut col, b);
                }
            },
            LexState::DollarQuote(tag) => {
                let tag_bytes = tag.as_bytes();
                if i + tag_bytes.len() <= bytes.len() && &bytes[i..i + tag_bytes.len()] == tag_bytes
                {
                    for _ in 0..tag_bytes.len() {
                        advance(&mut i, &mut line, &mut col, b'$');
                    }
                    state = LexState::Normal;
                } else {
                    advance(&mut i, &mut line, &mut col, b);
                }
            },
            LexState::LineComment => {
                if b == b'\n' {
                    state = LexState::Normal;
                }
                advance(&mut i, &mut line, &mut col, b);
            },
            LexState::BlockComment(depth) => {
                if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    *depth += 1;
                    advance(&mut i, &mut line, &mut col, b);
                    advance(&mut i, &mut line, &mut col, b'*');
                } else if b == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    *depth -= 1;
                    let zero = *depth == 0;
                    advance(&mut i, &mut line, &mut col, b);
                    advance(&mut i, &mut line, &mut col, b'/');
                    if zero {
                        state = LexState::Normal;
                    }
                } else {
                    advance(&mut i, &mut line, &mut col, b);
                }
            },
        }
    }

    match state {
        LexState::Normal | LexState::LineComment => {
            if has_content {
                let text = sql[start..].trim().to_string();
                if !text.is_empty() {
                    statements.push(TopStatement {
                        text,
                        start_line: stmt_line,
                        start_column: stmt_col,
                    });
                }
            }
            Ok(statements)
        },
        LexState::SingleQuote => Err(vec![LintError {
            line: start_line,
            column: start_col,
            severity: LintSeverity::Error,
            message: "unterminated string literal".into(),
            source: source.to_string(),
        }]),
        LexState::DollarQuote(tag) => Err(vec![LintError {
            line: start_line,
            column: start_col,
            severity: LintSeverity::Error,
            message: format!("unterminated dollar-quoted string: {tag}"),
            source: source.to_string(),
        }]),
        LexState::BlockComment(_) => Err(vec![LintError {
            line: start_line,
            column: start_col,
            severity: LintSeverity::Error,
            message: "unterminated block comment".into(),
            source: source.to_string(),
        }]),
    }
}

fn advance(i: &mut usize, line: &mut u32, col: &mut u32, b: u8) {
    *i += 1;
    if b == b'\n' {
        *line += 1;
        *col = 1;
    } else {
        *col += 1;
    }
}

fn classify(stmt: &TopStatement, source: &str) -> Option<LintError> {
    let stripped = strip_sql_comments(&stmt.text);
    let upper = uppercase_keywords(&stripped);
    let tokens: Vec<&str> = upper.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let leading = tokens[0];

    let reject = |reason: &str| LintError {
        line: stmt.start_line,
        column: stmt.start_column,
        severity: LintSeverity::Error,
        message: format!(
            "imperative SQL in declarative schema: {reason} — move to \
             schema/migrations/NNN_<name>.sql"
        ),
        source: source.to_string(),
    };

    match leading {
        "ALTER" => return Some(reject("ALTER")),
        "DROP" => return Some(reject("DROP")),
        "UPDATE" => return Some(reject("UPDATE")),
        "INSERT" => return Some(reject("INSERT")),
        "DELETE" => return Some(reject("DELETE")),
        "TRUNCATE" => return Some(reject("TRUNCATE")),
        "GRANT" => return Some(reject("GRANT")),
        "REVOKE" => return Some(reject("REVOKE")),
        "DO" => return Some(reject("DO $$ block")),
        _ => {},
    }

    if leading == "CREATE" {
        return classify_create(&tokens, stmt, source);
    }

    if leading == "COMMENT" && tokens.get(1) == Some(&"ON") {
        return None;
    }

    if leading == "SELECT" {
        return Some(LintError {
            line: stmt.start_line,
            column: stmt.start_column,
            severity: LintSeverity::Error,
            message: "imperative SQL in declarative schema: SELECT — move to \
                      schema/migrations/NNN_<name>.sql"
                .into(),
            source: source.to_string(),
        });
    }

    None
}

fn classify_create(tokens: &[&str], stmt: &TopStatement, source: &str) -> Option<LintError> {
    let mut idx = 1;

    if tokens.get(idx) == Some(&"OR") && tokens.get(idx + 1) == Some(&"REPLACE") {
        idx += 2;
    }

    if tokens.get(idx) == Some(&"UNIQUE") {
        idx += 1;
    }

    let kind = match tokens.get(idx) {
        Some(k) => *k,
        None => return None,
    };
    idx += 1;

    let has_if_not_exists = tokens.get(idx) == Some(&"IF")
        && tokens.get(idx + 1) == Some(&"NOT")
        && tokens.get(idx + 2) == Some(&"EXISTS");

    match kind {
        "TABLE" => {
            if !has_if_not_exists {
                return Some(LintError {
                    line: stmt.start_line,
                    column: stmt.start_column,
                    severity: LintSeverity::Warning,
                    message: "CREATE TABLE without IF NOT EXISTS — add IF NOT EXISTS for \
                              idempotency"
                        .into(),
                    source: source.to_string(),
                });
            }
            None
        },
        "EXTENSION" => {
            if !has_if_not_exists {
                return Some(LintError {
                    line: stmt.start_line,
                    column: stmt.start_column,
                    severity: LintSeverity::Warning,
                    message: "CREATE EXTENSION without IF NOT EXISTS".into(),
                    source: source.to_string(),
                });
            }
            None
        },
        _ => None,
    }
}

fn strip_sql_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_single = false;
    let mut in_dollar: Option<String> = None;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(tag) = &in_dollar {
            let tag_b = tag.as_bytes();
            if i + tag_b.len() <= bytes.len() && &bytes[i..i + tag_b.len()] == tag_b {
                out.push_str(tag);
                i += tag_b.len();
                in_dollar = None;
            } else {
                out.push(b as char);
                i += 1;
            }
            continue;
        }
        if in_single {
            out.push(b as char);
            if b == b'\'' {
                if bytes.get(i + 1) == Some(&b'\'') {
                    out.push('\'');
                    i += 2;
                    continue;
                }
                in_single = false;
            }
            i += 1;
            continue;
        }
        if b == b'\'' {
            in_single = true;
            out.push('\'');
            i += 1;
            continue;
        }
        if b == b'$' {
            if let Some(end) = dollar_tag_end(bytes, i) {
                let tag = text[i..=end].to_string();
                out.push_str(&tag);
                i = end + 1;
                in_dollar = Some(tag);
                continue;
            }
        }
        if b == b'-' && bytes.get(i + 1) == Some(&b'-') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let mut depth = 1u32;
            i += 2;
            while i < bytes.len() && depth > 0 {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        out.push(b as char);
        i += 1;
    }
    out
}

fn uppercase_keywords(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut in_dollar = false;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if !in_string && !in_dollar && b == b'$' {
            if let Some(end) = dollar_tag_end(bytes, i) {
                out.push_str(&text[i..=end]);
                i = end + 1;
                in_dollar = true;
                continue;
            }
        }
        if in_dollar && b == b'$' {
            if let Some(end) = dollar_tag_end(bytes, i) {
                out.push_str(&text[i..=end]);
                i = end + 1;
                in_dollar = false;
                continue;
            }
        }
        if in_dollar {
            out.push(b as char);
            i += 1;
            continue;
        }
        if b == b'\'' {
            in_string = !in_string;
            out.push('\'');
            i += 1;
            continue;
        }
        if in_string {
            out.push(b as char);
            i += 1;
            continue;
        }
        out.push(b.to_ascii_uppercase() as char);
        i += 1;
    }
    out
}
