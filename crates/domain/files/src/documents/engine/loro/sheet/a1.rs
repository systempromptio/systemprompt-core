//! A1 addresses and sheet node ids, independent of any codec.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[must_use]
pub fn column_letters(mut col: u32) -> String {
    let mut letters = Vec::new();
    while col > 0 {
        let rem = (col - 1) % 26;
        letters.push(char::from(b'A' + rem as u8));
        col = (col - 1) / 26;
    }
    letters.iter().rev().collect()
}

#[must_use]
pub fn a1(col: u32, row: u32) -> String {
    format!("{}{row}", column_letters(col))
}

#[must_use]
pub fn parse_a1(text: &str) -> Option<(u32, u32)> {
    let text = text.trim().replace('$', "");
    let split = text.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = text.split_at(split);
    if letters.is_empty() || letters.len() > 3 || !letters.chars().all(|c| c.is_ascii_alphabetic())
    {
        return None;
    }
    let col = letters.chars().fold(0u32, |acc, c| {
        acc * 26 + u32::from(c.to_ascii_uppercase() as u8 - b'A' + 1)
    });
    let row = digits.parse::<u32>().ok().filter(|row| *row > 0)?;
    Some((col, row))
}

/// A normalised 1-based rectangle, `start` top-left and `end` bottom-right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A1Range {
    pub start: (u32, u32),
    pub end: (u32, u32),
}

impl A1Range {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (start, end) = if let Some((a, b)) = text.split_once(':') {
            (parse_a1(a)?, parse_a1(b)?)
        } else {
            let only = parse_a1(text)?;
            (only, only)
        };
        Some(Self {
            start: (start.0.min(end.0), start.1.min(end.1)),
            end: (start.0.max(end.0), start.1.max(end.1)),
        })
    }
}

/// What a projection node id (`sheet[<name>]`, `…/row[<n>]`, `…/<A1>`) names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetTarget {
    Sheet,
    Row(u32),
    Cell(u32, u32),
}

#[must_use]
pub fn parse_node(id: &str) -> Option<(String, SheetTarget)> {
    let rest = id.strip_prefix("sheet[")?;
    let mut name = String::new();
    let mut chars = rest.char_indices();
    let mut tail_start = None;
    while let Some((index, c)) = chars.next() {
        match c {
            '\\' if rest[index + 1..].starts_with(']') => {
                name.push(']');
                chars.next();
            },
            ']' => {
                tail_start = Some(index + 1);
                break;
            },
            other => name.push(other),
        }
    }
    let tail = &rest[tail_start?..];
    if tail.is_empty() {
        return Some((name, SheetTarget::Sheet));
    }
    let tail = tail.strip_prefix('/')?;
    if let Some(row) = tail.strip_prefix("row[").and_then(|t| t.strip_suffix(']')) {
        return Some((name, SheetTarget::Row(row.parse().ok().filter(|r| *r > 0)?)));
    }
    let (col, row) = parse_a1(tail)?;
    Some((name, SheetTarget::Cell(col, row)))
}
