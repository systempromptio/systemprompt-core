use thiserror::Error;

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

#[derive(Debug, Clone, Copy, Error)]
pub enum AdminSqlError {
    #[error("SQL query is empty")]
    Empty,
    #[error("SQL query contains multiple statements; only one is allowed")]
    MultipleStatements,
    #[error("SQL query must begin with SELECT, WITH, EXPLAIN, SHOW, TABLE, or VALUES")]
    NotReadOnly,
    #[error("SQL query contains forbidden keyword for read-only mode")]
    ForbiddenKeyword,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminSql(String);

impl AdminSql {
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
