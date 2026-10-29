//! The four standard PDF fonts the renderer draws with, their advance
//! widths and the `WinAnsiEncoding` bytes a string is shown as.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pdf_writer::Name;

const FALLBACK_WIDTH: u16 = 556;
const MONO_WIDTH: u16 = 600;
const REPLACEMENT: u8 = b'?';

const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Font {
    Sans,
    SansBold,
    SansItalic,
    Mono,
}

impl Font {
    pub(super) const ALL: [Self; 4] = [Self::Sans, Self::SansBold, Self::SansItalic, Self::Mono];

    #[must_use]
    pub(super) const fn base_font(self) -> Name<'static> {
        match self {
            Self::Sans => Name(b"Helvetica"),
            Self::SansBold => Name(b"Helvetica-Bold"),
            Self::SansItalic => Name(b"Helvetica-Oblique"),
            Self::Mono => Name(b"Courier"),
        }
    }

    #[must_use]
    pub(super) const fn resource(self) -> Name<'static> {
        match self {
            Self::Sans => Name(b"F1"),
            Self::SansBold => Name(b"F2"),
            Self::SansItalic => Name(b"F3"),
            Self::Mono => Name(b"F4"),
        }
    }

    fn glyph_width(self, c: char) -> u16 {
        if self == Self::Mono {
            return MONO_WIDTH;
        }
        let table = if self == Self::SansBold {
            &HELVETICA_BOLD
        } else {
            &HELVETICA
        };
        u32::from(c)
            .checked_sub(0x20)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| table.get(index))
            .copied()
            .unwrap_or(FALLBACK_WIDTH)
    }

    #[must_use]
    pub(super) fn text_width(self, text: &str, size: f32) -> f32 {
        let units: u32 = text.chars().map(|c| u32::from(self.glyph_width(c))).sum();
        units_to_points(units) * size
    }
}

fn units_to_points(units: u32) -> f32 {
    let whole = u16::try_from(units / 1000).unwrap_or(u16::MAX);
    let rest = u16::try_from(units % 1000).unwrap_or(0);
    f32::from(whole) + f32::from(rest) / 1000.0
}

fn win_ansi(c: char) -> u8 {
    match c {
        '\u{20}'..='\u{7e}' | '\u{a0}'..='\u{ff}' => {
            u8::try_from(u32::from(c)).unwrap_or(REPLACEMENT)
        },
        '\u{20ac}' => 0x80,
        '\u{2026}' => 0x85,
        '\u{2018}' => 0x91,
        '\u{2019}' => 0x92,
        '\u{201c}' => 0x93,
        '\u{201d}' => 0x94,
        '\u{2022}' => 0x95,
        '\u{2013}' => 0x96,
        '\u{2014}' => 0x97,
        '\u{2122}' => 0x99,
        _ => REPLACEMENT,
    }
}

#[must_use]
pub(super) fn encode(text: &str) -> Vec<u8> {
    text.chars().map(win_ansi).collect()
}
