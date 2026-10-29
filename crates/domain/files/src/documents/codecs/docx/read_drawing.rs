//! A `w:drawing` read as an image block, and the pixel size and type of
//! image bytes.
//!
//! The image's blob is `docx-media:<target>`, its picture part relative to
//! `word/`, so an unchanged image is written back by reference. Extents
//! are EMUs (9525 per CSS pixel). An anchored drawing's wrap element names
//! its wrap; `wrapNone` is behind or in front of the text by `behindDoc`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{Cursor, Read};

use systemprompt_provider_contracts::documents::content_hash;
use zip::ZipArchive;

use super::read::ReadCtx;
use crate::documents::codecs::xml_tree::Element;
use crate::documents::model::BlockKind;
use crate::documents::model::layout::ImageWrap;

pub(super) const EMU_PER_PX: u64 = 9525;

pub(super) const MEDIA_SCHEME: &str = "docx-media:";

pub(super) fn image(drawing: &Element, ctx: &ReadCtx) -> Option<BlockKind> {
    let (frame, wrap) = match (drawing.child("wp:inline"), drawing.child("wp:anchor")) {
        (Some(inline), _) => (inline, ImageWrap::Inline),
        (None, Some(anchor)) => (anchor, anchor_wrap(anchor)),
        (None, None) => return None,
    };
    let embed = frame.find("a:blip")?.attr("r:embed")?;
    let target = ctx.rels.get(embed)?;
    let extent = frame.child("wp:extent");
    let px = |name: &str| {
        extent
            .and_then(|e| e.attr(name))
            .and_then(|v| v.parse::<u64>().ok())
            .and_then(|emu| u32::try_from(emu / EMU_PER_PX).ok())
    };
    let alt = frame
        .child("wp:docPr")
        .and_then(|d| d.attr("descr").or_else(|| d.attr("title")))
        .unwrap_or_default();
    Some(BlockKind::Image {
        blob: format!("{MEDIA_SCHEME}{target}"),
        alt: alt.to_owned(),
        width: px("cx"),
        height: px("cy"),
        caption: None,
        wrap,
    })
}

fn anchor_wrap(anchor: &Element) -> ImageWrap {
    let has = |name: &str| anchor.child(name).is_some();
    if has("wp:wrapTopAndBottom") {
        ImageWrap::TopBottom
    } else if has("wp:wrapSquare") || has("wp:wrapTight") || has("wp:wrapThrough") {
        ImageWrap::Square
    } else if anchor.attr("behindDoc") == Some("1") {
        ImageWrap::Behind
    } else {
        ImageWrap::InFront
    }
}

fn be16(bytes: &[u8], at: usize) -> Option<u32> {
    let pair: [u8; 2] = bytes.get(at..at + 2)?.try_into().ok()?;
    Some(u32::from(u16::from_be_bytes(pair)))
}

fn le16(bytes: &[u8], at: usize) -> Option<u32> {
    let pair: [u8; 2] = bytes.get(at..at + 2)?.try_into().ok()?;
    Some(u32::from(u16::from_le_bytes(pair)))
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    let quad: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
    Some(u32::from_be_bytes(quad))
}

pub(super) fn pixel_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(b"\x89PNG") {
        return Some((be32(bytes, 16)?, be32(bytes, 20)?));
    }
    if bytes.starts_with(b"GIF8") {
        return Some((le16(bytes, 6)?, le16(bytes, 8)?));
    }
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut at = 2;
    while at + 9 < bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }
        let marker = bytes[at + 1];
        let len = usize::try_from(be16(bytes, at + 2)?).ok()?;
        if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
            return Some((be16(bytes, at + 7)?, be16(bytes, at + 5)?));
        }
        at += 2 + len;
    }
    None
}

pub(super) fn image_type(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"\x89PNG") {
        Some(("png", "image/png"))
    } else if bytes.starts_with(b"GIF8") {
        Some(("gif", "image/gif"))
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        Some(("jpeg", "image/jpeg"))
    } else {
        None
    }
}

pub const MAX_PART_BYTES: u64 = 64 * 1024 * 1024;

pub fn saved_upload_hashes(bytes: &[u8], wanted: &[String], limit: u64) -> Vec<String> {
    let Ok(mut archive) = ZipArchive::new(Cursor::new(bytes)) else {
        return Vec::new();
    };
    #[expect(
        clippy::needless_collect,
        reason = "the names borrow the archive, which by_name then borrows mutably"
    )]
    let names: Vec<(String, String)> = archive
        .file_names()
        .filter_map(Result::ok)
        .filter_map(|name| {
            let stem = name.strip_prefix("word/media/sp-")?.split('.').next()?;
            let wanted = is_hash(stem) && wanted.iter().any(|w| w == stem);
            wanted.then(|| (name.to_string(), stem.to_owned()))
        })
        .collect();
    names
        .into_iter()
        .filter_map(|(name, stem)| {
            let file = archive.by_name(&name).ok()?;
            let content = read_capped(file, limit)?;
            (content_hash(&content).as_str() == stem).then_some(stem)
        })
        .collect()
}

pub(super) fn saved_upload_part(bytes: &[u8], hash: &str, limit: u64) -> Option<(String, Vec<u8>)> {
    if !is_hash(hash) {
        return None;
    }
    let mut archive = ZipArchive::new(Cursor::new(bytes)).ok()?;
    ["png", "jpeg", "gif"].iter().find_map(|extension| {
        let target = format!("media/sp-{hash}.{extension}");
        let file = archive.by_name(&format!("word/{target}")).ok()?;
        let content = read_capped(file, limit)?;
        (content_hash(&content).as_str() == hash).then_some((target, content))
    })
}

fn read_capped(file: impl Read, limit: u64) -> Option<Vec<u8>> {
    let mut content = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut content)
        .ok()?;
    (u64::try_from(content.len()).ok()? <= limit).then_some(content)
}

fn is_hash(stem: &str) -> bool {
    stem.len() == 64 && stem.chars().all(|c| c.is_ascii_hexdigit())
}

/// The bytes and type of an image the package holds: a `docx-media:` part
/// under `word/`, or an upload a write saved as `media/sp-<hash>`.
#[must_use]
pub fn held_image(bytes: &[u8], reference: &str, limit: u64) -> Option<(String, Vec<u8>)> {
    let content = match reference.strip_prefix(MEDIA_SCHEME) {
        Some(target) if !target.split('/').any(|seg| seg == ".." || seg.is_empty()) => {
            let mut archive = ZipArchive::new(Cursor::new(bytes)).ok()?;
            let file = archive.by_name(&format!("word/{target}")).ok()?;
            read_capped(file, limit)?
        },
        Some(_) => return None,
        None => saved_upload_part(bytes, reference, limit)?.1,
    };
    let (_, mime) = image_type(&content)?;
    Some((mime.to_owned(), content))
}
