//! The bundled `DejaVu` faces the renderer draws with, their metrics and the
//! character each one can show.
//!
//! The faces are cut down to Latin, Greek, Cyrillic, punctuation and common
//! symbols (`fonts_data/LICENSE` holds their licence). A character no face
//! holds is drawn as U+FFFD, so a gap in coverage is visible on the page.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::OnceLock;

use ttf_parser::{Face, GlyphId};

pub(super) const REPLACEMENT: char = '\u{fffd}';

const SANS: &[u8] = include_bytes!("fonts_data/DejaVuSans.ttf");
const SANS_BOLD: &[u8] = include_bytes!("fonts_data/DejaVuSans-Bold.ttf");
const SANS_ITALIC: &[u8] = include_bytes!("fonts_data/DejaVuSans-Oblique.ttf");
const SANS_BOLD_ITALIC: &[u8] = include_bytes!("fonts_data/DejaVuSans-BoldOblique.ttf");
const MONO: &[u8] = include_bytes!("fonts_data/DejaVuSansMono.ttf");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum Font {
    Sans,
    SansBold,
    SansItalic,
    SansBoldItalic,
    Mono,
}

static FACES: [OnceLock<Option<Face<'static>>>; 5] = [const { OnceLock::new() }; 5];

impl Font {
    #[must_use]
    pub(super) const fn styled(bold: bool, italic: bool, mono: bool) -> Self {
        match (mono, bold, italic) {
            (true, ..) => Self::Mono,
            (false, true, true) => Self::SansBoldItalic,
            (false, true, false) => Self::SansBold,
            (false, false, true) => Self::SansItalic,
            (false, false, false) => Self::Sans,
        }
    }

    #[must_use]
    pub(super) const fn data(self) -> &'static [u8] {
        match self {
            Self::Sans => SANS,
            Self::SansBold => SANS_BOLD,
            Self::SansItalic => SANS_ITALIC,
            Self::SansBoldItalic => SANS_BOLD_ITALIC,
            Self::Mono => MONO,
        }
    }

    #[must_use]
    pub(super) const fn postscript_name(self) -> &'static str {
        match self {
            Self::Sans => "DejaVuSans",
            Self::SansBold => "DejaVuSans-Bold",
            Self::SansItalic => "DejaVuSans-Oblique",
            Self::SansBoldItalic => "DejaVuSans-BoldOblique",
            Self::Mono => "DejaVuSansMono",
        }
    }

    #[must_use]
    pub(super) const fn resource(self) -> &'static str {
        match self {
            Self::Sans => "F1",
            Self::SansBold => "F2",
            Self::SansItalic => "F3",
            Self::SansBoldItalic => "F4",
            Self::Mono => "F5",
        }
    }

    pub(super) fn face(self) -> Option<&'static Face<'static>> {
        FACES[self as usize]
            .get_or_init(|| Face::parse(self.data(), 0).ok())
            .as_ref()
    }

    #[must_use]
    pub(super) fn drawn(self, c: char) -> char {
        let held = self.face().and_then(|face| face.glyph_index(c)).is_some();
        if held { c } else { REPLACEMENT }
    }

    #[must_use]
    pub(super) fn glyph(self, c: char) -> GlyphId {
        self.face()
            .and_then(|face| face.glyph_index(self.drawn(c)))
            .unwrap_or(GlyphId(0))
    }

    #[must_use]
    pub(super) fn glyph_advance(self, glyph: GlyphId) -> f32 {
        self.face().map_or(0.0, |face| {
            let units = face.glyph_hor_advance(glyph).unwrap_or(0);
            f32::from(units) * 1000.0 / f32::from(face.units_per_em())
        })
    }

    #[must_use]
    pub(super) fn char_width(self, c: char, size: f32) -> f32 {
        self.glyph_advance(self.glyph(c)) * size / 1000.0
    }

    #[must_use]
    pub(super) fn text_width(self, text: &str, size: f32) -> f32 {
        text.chars().map(|c| self.char_width(c, size)).sum()
    }
}
