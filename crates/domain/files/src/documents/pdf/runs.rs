//! A styled stretch of text, and the lowering of a block's marks to runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::model::{Mark, MarkKind, segments};

pub(super) type Rgb = [u8; 3];

pub(super) const LINK_BLUE: Rgb = [0x1a, 0x56, 0xb0];
const DEFAULT_HIGHLIGHT: Rgb = [0xff, 0xf2, 0x9e];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Run {
    pub(super) text: String,
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) mono: bool,
    pub(super) underline: bool,
    pub(super) strike: bool,
    pub(super) color: Option<Rgb>,
    pub(super) highlight: Option<Rgb>,
    pub(super) link: Option<String>,
}

impl Run {
    #[must_use]
    pub(super) fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub(super) fn same_style(&self, other: &Self) -> bool {
        Self {
            text: String::new(),
            ..self.clone()
        } == Self {
            text: String::new(),
            ..other.clone()
        }
    }
}

#[must_use]
pub(super) fn parse_hex(value: &str) -> Option<Rgb> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

fn highlight_of(value: &str) -> Rgb {
    parse_hex(value).unwrap_or_else(|| match value.to_ascii_lowercase().as_str() {
        "green" => [0xb7, 0xe8, 0xb0],
        "cyan" => [0xb0, 0xe8, 0xf0],
        "magenta" => [0xf0, 0xc0, 0xf0],
        "red" => [0xf5, 0xb5, 0xb5],
        "blue" => [0xb5, 0xc8, 0xf5],
        _ => DEFAULT_HIGHLIGHT,
    })
}

fn is_safe_link(href: &str) -> bool {
    ["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| href.starts_with(scheme))
}

fn apply(run: &mut Run, kind: &MarkKind) {
    match kind {
        MarkKind::Bold => run.bold = true,
        MarkKind::Italic => run.italic = true,
        MarkKind::Underline => run.underline = true,
        MarkKind::Strike => run.strike = true,
        MarkKind::Code => run.mono = true,
        MarkKind::Color { value } => run.color = parse_hex(value).or(run.color),
        MarkKind::Highlight { value } => run.highlight = Some(highlight_of(value)),
        MarkKind::Caps => run.text = run.text.to_uppercase(),
        MarkKind::Link { href } if is_safe_link(href) => {
            run.link = Some(href.clone());
            run.underline = true;
            run.color = run.color.or(Some(LINK_BLUE));
        },
        MarkKind::Link { .. }
        | MarkKind::Comment { .. }
        | MarkKind::Font { .. }
        | MarkKind::Size { .. }
        | MarkKind::Superscript
        | MarkKind::Subscript
        | MarkKind::Field { .. }
        | MarkKind::Footnote { .. } => {},
    }
}

#[must_use]
pub(super) fn runs_of(text: &str, marks: &[Mark]) -> Vec<Run> {
    segments(text, marks)
        .into_iter()
        .map(|(piece, active)| {
            let mut run = Run::plain(piece);
            for kind in active {
                apply(&mut run, kind);
            }
            run
        })
        .collect()
}
