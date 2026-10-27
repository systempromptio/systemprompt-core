//! A1 cell references and sheet node ids for the `.ods` codec.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

pub(super) fn sheet_id(name: &str) -> String {
    format!("sheet[{}]", name.replace(']', "\\]"))
}

pub(super) fn column_letters(mut col: u32) -> String {
    let mut letters = Vec::new();
    while col > 0 {
        let rem = (col - 1) % 26;
        letters.push(char::from(b'A' + u8::try_from(rem).unwrap_or(0)));
        col = (col - 1) / 26;
    }
    letters.iter().rev().collect()
}

pub(super) fn a1(col: u32, row: u32) -> String {
    format!("{}{row}", column_letters(col))
}

pub(super) fn parse_a1(text: &str) -> Option<(u32, u32)> {
    let text = text.trim().replace('$', "");
    let split = text.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = text.split_at(split);
    if letters.is_empty() || letters.len() > 3 || !letters.chars().all(|c| c.is_ascii_alphabetic())
    {
        return None;
    }
    let col = letters
        .to_ascii_uppercase()
        .bytes()
        .fold(0u32, |acc, b| acc * 26 + u32::from(b - b'A' + 1));
    let row = digits.parse::<u32>().ok().filter(|r| *r > 0)?;
    Some((col, row))
}

pub(super) fn parse_range(text: &str) -> Option<(Range<u32>, Range<u32>)> {
    let (start, end) = text.split_once(':').unwrap_or((text, text));
    let (c0, r0) = parse_a1(start)?;
    let (c1, r1) = parse_a1(end)?;
    Some((c0.min(c1)..c0.max(c1) + 1, r0.min(r1)..r0.max(r1) + 1))
}
