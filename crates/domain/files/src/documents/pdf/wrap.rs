//! Greedy word wrapping measured with the standard-font advance widths;
//! a word wider than the line is broken at the character that overflows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::fonts::Font;

#[must_use]
pub(super) fn wrap(text: &str, font: Font, size: f32, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        for piece in split_long(word, font, size, width) {
            let candidate = if current.is_empty() {
                piece.clone()
            } else {
                format!("{current} {piece}")
            };
            if font.text_width(&candidate, size) <= width || current.is_empty() {
                current = candidate;
            } else {
                lines.push(std::mem::replace(&mut current, piece));
            }
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

fn split_long(word: &str, font: Font, size: f32, width: f32) -> Vec<String> {
    if font.text_width(word, size) <= width {
        return vec![word.to_owned()];
    }
    let mut pieces = Vec::new();
    let mut piece = String::new();
    for c in word.chars() {
        piece.push(c);
        if font.text_width(&piece, size) > width && piece.chars().count() > 1 {
            piece.pop();
            pieces.push(std::mem::take(&mut piece));
            piece.push(c);
        }
    }
    if !piece.is_empty() {
        pieces.push(piece);
    }
    pieces
}
