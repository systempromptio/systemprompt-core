//! Extract `CREATE TABLE` blocks and their column declarations from raw schema
//! SQL.
//!
//! The parser accepts the dialect we ship (well-formed Postgres DDL written by
//! us), not arbitrary SQL. Column-level constraints (`NOT NULL`, `DEFAULT …`,
//! `REFERENCES …`, `CHECK (…)`, `UNIQUE`, `PRIMARY KEY`, `GENERATED …`,
//! `COLLATE …`) are stripped from emitted type text. Table-level constraints
//! are skipped entirely.

use super::lexer::{
    LexState, find_matching_close_paren, is_top_level, is_word_boundary_before, read_identifier,
    skip_whitespace_and_comments, starts_with_keyword,
};
use super::{DeclaredColumn, DeclaredTable};

const TABLE_LEVEL_KEYWORDS: &[&str] = &[
    "CONSTRAINT",
    "PRIMARY",
    "FOREIGN",
    "UNIQUE",
    "CHECK",
    "EXCLUDE",
    "LIKE",
];

const COLUMN_CONSTRAINT_STARTERS: &[&str] = &[
    "NOT",
    "NULL",
    "PRIMARY",
    "REFERENCES",
    "DEFAULT",
    "CHECK",
    "UNIQUE",
    "GENERATED",
    "COLLATE",
    "CONSTRAINT",
];

/// Parse every `CREATE TABLE` block in `sql` and return the tables with their
/// declared columns.
///
/// Skips table-level constraints (`CONSTRAINT …`, `PRIMARY KEY (…)`,
/// `FOREIGN KEY (…)`, `UNIQUE (…)`, `CHECK (…)`, `EXCLUDE …`). Column-level
/// constraints (`NOT NULL`, `DEFAULT …`, `REFERENCES …`, `CHECK (…)`, `UNIQUE`,
/// `PRIMARY KEY`, `GENERATED …`, `COLLATE …`) are stripped from the emitted
/// `type_text`.
pub fn parse_declared_tables(sql: &str) -> Vec<DeclaredTable> {
    let bytes = sql.as_bytes();
    let mut tables = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        match find_create_table(bytes, i) {
            Some((name, body_start, body_end, after)) => {
                let body = &sql[body_start..body_end];
                let columns = parse_table_body(body);
                tables.push(DeclaredTable { name, columns });
                i = after;
            },
            None => break,
        }
    }
    tables
}

/// Locate the next `CREATE TABLE [IF NOT EXISTS] <ident> (…)` starting at or
/// after `start`. Returns `(table_name, body_start, body_end, after_close)`
/// where `body_*` brackets the inside of the parenthesised column list and
/// `after_close` is the byte after the matching `)`.
fn find_create_table(bytes: &[u8], start: usize) -> Option<(String, usize, usize, usize)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let upper = text.to_ascii_uppercase();
    let mut search = start;
    loop {
        let rel = upper[search..].find("CREATE")?;
        let pos = search + rel;
        if !is_top_level(bytes, pos) {
            search = pos + 6;
            continue;
        }
        if !is_word_boundary_before(bytes, pos) {
            search = pos + 6;
            continue;
        }
        let mut cursor = pos + "CREATE".len();
        cursor = skip_whitespace_and_comments(bytes, cursor);
        if !starts_with_keyword(&upper, cursor, "TABLE") {
            search = pos + 6;
            continue;
        }
        cursor += "TABLE".len();
        cursor = skip_whitespace_and_comments(bytes, cursor);
        if starts_with_keyword(&upper, cursor, "IF") {
            cursor += 2;
            cursor = skip_whitespace_and_comments(bytes, cursor);
            if !starts_with_keyword(&upper, cursor, "NOT") {
                search = pos + 6;
                continue;
            }
            cursor += 3;
            cursor = skip_whitespace_and_comments(bytes, cursor);
            if !starts_with_keyword(&upper, cursor, "EXISTS") {
                search = pos + 6;
                continue;
            }
            cursor += 6;
            cursor = skip_whitespace_and_comments(bytes, cursor);
        }
        let (name, after_name) = read_identifier(bytes, cursor)?;
        let mut after = skip_whitespace_and_comments(bytes, after_name);
        if bytes.get(after) != Some(&b'(') {
            search = pos + 6;
            continue;
        }
        let body_start = after + 1;
        let body_end = find_matching_close_paren(bytes, body_start)?;
        after = body_end + 1;
        return Some((name, body_start, body_end, after));
    }
}

fn parse_table_body(body: &str) -> Vec<DeclaredColumn> {
    split_top_level_commas(body)
        .iter()
        .filter_map(|s| parse_column_entry(s))
        .collect()
}

fn split_top_level_commas(body: &str) -> Vec<String> {
    let bytes = body.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut depth = 0u32;
    let mut state = LexState::Normal;
    while i < bytes.len() {
        match &mut state {
            LexState::Normal => match bytes[i] {
                b'(' => {
                    depth += 1;
                    i += 1;
                },
                b')' => {
                    depth = depth.saturating_sub(1);
                    i += 1;
                },
                b',' if depth == 0 => {
                    parts.push(body[start..i].to_string());
                    i += 1;
                    start = i;
                },
                b'\'' => {
                    state = LexState::SingleQuote;
                    i += 1;
                },
                b'-' if bytes.get(i + 1) == Some(&b'-') => {
                    state = LexState::LineComment;
                    i += 2;
                },
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    state = LexState::BlockComment(1);
                    i += 2;
                },
                _ => i += 1,
            },
            LexState::SingleQuote => {
                if bytes[i] == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        i += 2;
                    } else {
                        state = LexState::Normal;
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            },
            LexState::DollarQuote(_) => i += 1,
            LexState::LineComment => {
                if bytes[i] == b'\n' {
                    state = LexState::Normal;
                }
                i += 1;
            },
            LexState::BlockComment(d) => {
                if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    *d -= 1;
                    i += 2;
                    if *d == 0 {
                        state = LexState::Normal;
                    }
                } else {
                    i += 1;
                }
            },
        }
    }
    let tail = body[start..].trim();
    if !tail.is_empty() {
        parts.push(body[start..].to_string());
    }
    parts
}

fn parse_column_entry(raw: &str) -> Option<DeclaredColumn> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let leading_word: String = trimmed
        .chars()
        .take_while(|c| c.is_ascii_alphabetic() || *c == '_')
        .collect::<String>()
        .to_ascii_uppercase();
    if TABLE_LEVEL_KEYWORDS.contains(&leading_word.as_str()) {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let mut cursor = 0usize;
    let (name, after_name) = read_identifier(bytes, cursor)?;
    cursor = after_name;
    cursor = skip_whitespace_and_comments(bytes, cursor);
    let type_start = cursor;
    let mut type_end = cursor;
    while cursor < bytes.len() {
        cursor = skip_whitespace_and_comments(bytes, cursor);
        if cursor >= bytes.len() {
            break;
        }
        if bytes[cursor] == b'(' {
            let close = find_matching_close_paren(bytes, cursor + 1)?;
            cursor = close + 1;
            type_end = cursor;
            continue;
        }
        let word_start = cursor;
        while cursor < bytes.len()
            && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
        {
            cursor += 1;
        }
        if cursor == word_start {
            break;
        }
        let word = std::str::from_utf8(&bytes[word_start..cursor])
            .ok()?
            .to_ascii_uppercase();
        if COLUMN_CONSTRAINT_STARTERS.contains(&word.as_str()) {
            break;
        }
        type_end = cursor;
    }
    let type_text = trimmed[type_start..type_end].trim().to_string();
    if type_text.is_empty() {
        return None;
    }
    Some(DeclaredColumn { name, type_text })
}
