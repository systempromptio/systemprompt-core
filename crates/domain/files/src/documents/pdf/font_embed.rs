//! Writes one used face as a subset Type 0 font: the glyphs its text needs,
//! their widths and a `ToUnicode` map, so the text can be selected and copied.
//! Glyph order and the subset tag follow from the text alone, so the same
//! text writes the same bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Filter, Finish, Name, Pdf, Rect, Ref, Str};
use sha2::{Digest, Sha256};
use subsetter::{GlyphRemapper, subset};
use ttf_parser::GlyphId;

use super::compress;
use super::fonts::Font;

/// A face's subset: the CID each character it draws is shown as.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct Subset {
    cids: BTreeMap<char, u16>,
}

impl Subset {
    #[must_use]
    pub(super) fn encode(&self, font: Font, text: &str) -> Vec<u8> {
        text.chars()
            .filter_map(|c| self.cids.get(&font.drawn(c)))
            .flat_map(|cid| cid.to_be_bytes())
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FontRefs {
    pub(super) type0: Ref,
    pub(super) cid: Ref,
    pub(super) descriptor: Ref,
    pub(super) file: Ref,
    pub(super) cmap: Ref,
}

fn subset_tag(font: Font, cids: &BTreeMap<char, u16>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(font.postscript_name());
    for c in cids.keys() {
        hasher.update(u32::from(*c).to_be_bytes());
    }
    hasher
        .finalize()
        .iter()
        .take(6)
        .map(|byte| char::from(b'A' + byte % 26))
        .collect()
}

fn scaled(font: Font, units: i16) -> f32 {
    font.face().map_or(0.0, |face| {
        f32::from(units) * 1000.0 / f32::from(face.units_per_em())
    })
}

pub(super) fn embed(
    pdf: &mut Pdf,
    font: Font,
    used: &BTreeSet<char>,
    refs: FontRefs,
) -> Result<Subset, std::io::Error> {
    let drawn: BTreeSet<char> = used.iter().map(|c| font.drawn(*c)).collect();
    let mut remapper = GlyphRemapper::new();
    let cids: BTreeMap<char, u16> = drawn
        .iter()
        .map(|c| (*c, remapper.remap(font.glyph(*c).0)))
        .collect();
    let file = subset(font.data(), 0, &remapper)
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    let name = format!("{}+{}", subset_tag(font, &cids), font.postscript_name());
    let base = Name(name.as_bytes());
    let info = SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    };

    pdf.type0_font(refs.type0)
        .base_font(base)
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(refs.cid)
        .to_unicode(refs.cmap);

    let widths: Vec<f32> = remapper
        .remapped_gids()
        .map(|gid| font.glyph_advance(GlyphId(gid)))
        .collect();
    let mut cid = pdf.cid_font(refs.cid);
    cid.subtype(CidFontType::Type2)
        .base_font(base)
        .system_info(info)
        .font_descriptor(refs.descriptor)
        .default_width(0.0)
        .cid_to_gid_map_predefined(Name(b"Identity"));
    cid.widths().consecutive(0, widths);
    cid.finish();

    write_descriptor(pdf, font, base, refs);

    let packed = compress(&file)?;
    pdf.stream(refs.file, &packed)
        .filter(Filter::FlateDecode)
        .pair(
            Name(b"Length1"),
            i32::try_from(file.len()).unwrap_or(i32::MAX),
        );

    let mut cmap = UnicodeCmap::new(Name(b"Custom"), info);
    for (c, cid) in &cids {
        cmap.pair(*cid, *c);
    }
    let cmap = cmap.finish();
    pdf.cmap(refs.cmap, cmap.as_slice());
    Ok(Subset { cids })
}

fn write_descriptor(pdf: &mut Pdf, font: Font, base: Name<'_>, refs: FontRefs) {
    let Some(face) = font.face() else {
        return;
    };
    let bbox = face.global_bounding_box();
    let mut flags = FontFlags::SYMBOLIC;
    if font == Font::Mono {
        flags |= FontFlags::FIXED_PITCH;
    }
    if matches!(font, Font::SansItalic | Font::SansBoldItalic) {
        flags |= FontFlags::ITALIC;
    }
    let stem = if matches!(font, Font::SansBold | Font::SansBoldItalic) {
        140.0
    } else {
        80.0
    };
    pdf.font_descriptor(refs.descriptor)
        .name(base)
        .flags(flags)
        .bbox(Rect::new(
            scaled(font, bbox.x_min),
            scaled(font, bbox.y_min),
            scaled(font, bbox.x_max),
            scaled(font, bbox.y_max),
        ))
        .italic_angle(face.italic_angle())
        .ascent(scaled(font, face.ascender()))
        .descent(scaled(font, face.descender()))
        .cap_height(scaled(
            font,
            face.capital_height().unwrap_or_else(|| face.ascender()),
        ))
        .stem_v(stem)
        .font_file2(refs.file);
}
