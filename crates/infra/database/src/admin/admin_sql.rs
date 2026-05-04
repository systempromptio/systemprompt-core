//! Parser/validator for admin-supplied SQL strings.
//!
//! Two parse modes are exposed:
//! - [`AdminSql::parse_readonly`] — only `SELECT`/`WITH`/`EXPLAIN`/`SHOW`/
//!   `TABLE`/`VALUES` queries with no forbidden keywords.
//! - [`AdminSql::parse_unrestricted`] — single statement, otherwise free-form.

use thiserror::Error;

/// Default row limit applied by the read-only [`AdminSql::parse_readonly`]
/// path when callers don't supply an explicit limit.
pub const DEFAULT_READONLY_ROW_LIMIT: usize = 1000;

const READONLY_PREFIXES: &[&str] = &["select", "with", "explain", "show", "table", "values"];

const FORBIDDEN_KEYWORDS: &[&str] = &[
    " drop ",
    " delete ",
    " insert ",
    " update ",
    " alter ",
    " create ",
    " truncate ",
    " grant ",
    " revoke ",
    " copy ",
    " vacuum ",
    " call ",
    " lock ",
    " set ",
    " reset ",
    " rename ",
];

/// Reasons admin SQL parsing can fail.
#[derive(Debug, Clone, Copy, Error)]
pub enum AdminSqlError {
    /// Input was empty after stripping comments and whitespace.
    #[error("SQL query is empty")]
    Empty,
    /// Multiple statements were detected separated by `;`.
    #[error("SQL query contains multiple statements; only one is allowed")]
    MultipleStatements,
    /// First keyword is not in the read-only allowlist.
    #[error("SQL query must begin with SELECT, WITH, EXPLAIN, SHOW, TABLE, or VALUES")]
    NotReadOnly,
    /// One of the forbidden DDL/DML keywords was found.
    #[error("SQL query contains forbidden keyword for read-only mode")]
    ForbiddenKeyword,
}

/// Validated SQL string ready to hand to a SQL executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminSql(String);

impl AdminSql {
    /// Parse a string as a read-only admin SQL statement. Strips comments,
    /// rejects multi-statement input, requires a read-only leading keyword,
    /// and rejects any embedded forbidden DDL/DML keyword.
    pub fn parse_readonly(raw: &str) -> Result<Self, AdminSqlError> {
        let stripped = strip_comments(raw);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            return Err(AdminSqlError::Empty);
        }

        let without_terminator = trimmed.strip_suffix(';').unwrap_or(trimmed).trim_end();
        if without_terminator.contains(';') {
            return Err(AdminSqlError::MultipleStatements);
        }

        let lower = without_terminator.to_lowercase();
        if !READONLY_PREFIXES
            .iter()
            .any(|p| starts_with_word(&lower, p))
        {
            return Err(AdminSqlError::NotReadOnly);
        }

        let padded = format!(" {lower} ");
        if FORBIDDEN_KEYWORDS.iter().any(|kw| padded.contains(kw)) {
            return Err(AdminSqlError::ForbiddenKeyword);
        }

        Ok(Self(without_terminator.to_string()))
    }

    /// Parse a string as an unrestricted single-statement admin SQL query.
    /// Strips comments and rejects multi-statement input but does not enforce
    /// a leading-keyword allowlist.
    pub fn parse_unrestricted(raw: &str) -> Result<Self, AdminSqlError> {
        let stripped = strip_comments(raw);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            return Err(AdminSqlError::Empty);
        }

        let without_terminator = trimmed.strip_suffix(';').unwrap_or(trimmed).trim_end();
        if without_terminator.contains(';') {
            return Err(AdminSqlError::MultipleStatements);
        }

        Ok(Self(without_terminator.to_string()))
    }

    /// Borrow the underlying validated SQL string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn strip_comments(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '-' && chars.peek() == Some(&'-') {
            for nc in chars.by_ref() {
                if nc == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut prev = '\0';
            for nc in chars.by_ref() {
                if prev == '*' && nc == '/' {
                    break;
                }
                prev = nc;
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn starts_with_word(haystack: &str, needle: &str) -> bool {
    if !haystack.starts_with(needle) {
        return false;
    }
    haystack[needle.len()..]
        .chars()
        .next()
        .is_none_or(|c| c.is_whitespace() || c == '(' || c == ';')
}
