//! Image uploads: a dimension cap and metadata stripping.
//!
//! PNG drops its `eXIf`, `tEXt`, `zTXt`, `iTXt` and `tIME` chunks; JPEG
//! drops its APP1 (EXIF, XMP), APP13 (IPTC) and comment segments; WebP
//! drops its `EXIF` and `XMP ` chunks and clears their `VP8X` flags. Pixel
//! data is copied as stored, not decoded and re-encoded, so no decoder
//! runs on untrusted input here. GIF carries no EXIF and is only measured.
//! An image whose header cannot be read, or whose width or height is past
//! the cap, is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::sniff::Kind;
use crate::config::UploadLimits;
use crate::uploads::error::{RefusalReason, UploadError, UploadResult};

const PNG_SIGNATURE_LEN: usize = 8;
const PNG_DROPPED: [&[u8; 4]; 5] = [b"eXIf", b"tEXt", b"zTXt", b"iTXt", b"tIME"];
const JPEG_DROPPED: [u8; 3] = [0xe1, 0xed, 0xfe];
const WEBP_DROPPED: [&[u8; 4]; 2] = [b"EXIF", b"XMP "];
const VP8X_METADATA_FLAGS: u8 = 0x08 | 0x04;

fn refuse(detail: impl Into<String>) -> UploadError {
    UploadError::refused(RefusalReason::ImageLimits, detail)
}

fn be32(bytes: &[u8], at: usize) -> Option<u64> {
    let b = bytes.get(at..at + 4)?;
    Some(u64::from(u32::from_be_bytes([b[0], b[1], b[2], b[3]])))
}

fn le(bytes: &[u8], at: usize, len: usize) -> Option<u64> {
    let b = bytes.get(at..at + len)?;
    Some(
        b.iter()
            .rev()
            .fold(0, |acc, byte| (acc << 8) | u64::from(*byte)),
    )
}

pub(crate) fn sanitize(kind: Kind, bytes: &[u8], limits: &UploadLimits) -> UploadResult<Vec<u8>> {
    let (stripped, dimensions) = match kind {
        Kind::Png => png(bytes),
        Kind::Jpeg => jpeg(bytes),
        Kind::Webp => webp(bytes),
        Kind::Gif => le(bytes, 6, 2)
            .zip(le(bytes, 8, 2))
            .map(|size| (bytes.to_vec(), size)),
        _ => None,
    }
    .ok_or_else(|| refuse("the image header could not be read"))?;
    let (width, height) = dimensions;
    let cap = limits.image_max_dimension;
    if width == 0 || height == 0 || width > cap || height > cap {
        return Err(refuse(format!(
            "{width}x{height} is outside the {cap}-pixel dimension cap"
        )));
    }
    Ok(stripped)
}

fn png(bytes: &[u8]) -> Option<(Vec<u8>, (u64, u64))> {
    let mut out = bytes.get(..PNG_SIGNATURE_LEN)?.to_vec();
    let mut at = PNG_SIGNATURE_LEN;
    let mut dimensions = None;
    while at < bytes.len() {
        let len = usize::try_from(be32(bytes, at)?).ok()?;
        let kind = bytes.get(at + 4..at + 8)?;
        let end = at.checked_add(12)?.checked_add(len)?;
        let chunk = bytes.get(at..end)?;
        if kind == b"IHDR" {
            dimensions = Some((be32(bytes, at + 8)?, be32(bytes, at + 12)?));
        }
        if !PNG_DROPPED.iter().any(|dropped| kind == &dropped[..]) {
            out.extend_from_slice(chunk);
        }
        at = end;
        if kind == b"IEND" {
            break;
        }
    }
    Some((out, dimensions?))
}

fn jpeg(bytes: &[u8]) -> Option<(Vec<u8>, (u64, u64))> {
    let mut out = bytes.get(..2)?.to_vec();
    let mut at = 2;
    let mut dimensions = None;
    loop {
        if *bytes.get(at)? != 0xff {
            return None;
        }
        let marker = *bytes.get(at + 1)?;
        if marker == 0xda {
            out.extend_from_slice(bytes.get(at..)?);
            break;
        }
        let len = usize::try_from(be16(bytes, at + 2)?).ok()?;
        let end = at.checked_add(2)?.checked_add(len)?;
        let segment = bytes.get(at..end)?;
        let frame = (0xc0..=0xcf).contains(&marker) && ![0xc4, 0xc8, 0xcc].contains(&marker);
        if frame {
            dimensions = Some((be16(bytes, at + 7)?, be16(bytes, at + 5)?));
        }
        if !JPEG_DROPPED.contains(&marker) {
            out.extend_from_slice(segment);
        }
        at = end;
    }
    Some((out, dimensions?))
}

fn be16(bytes: &[u8], at: usize) -> Option<u64> {
    let b = bytes.get(at..at + 2)?;
    Some(u64::from(u16::from_be_bytes([b[0], b[1]])))
}

fn webp(bytes: &[u8]) -> Option<(Vec<u8>, (u64, u64))> {
    let mut out = bytes.get(..12)?.to_vec();
    let mut at = 12;
    let mut dimensions = None;
    while at < bytes.len() {
        let kind = bytes.get(at..at + 4)?;
        let len = usize::try_from(le(bytes, at + 4, 4)?).ok()?;
        let end = at.checked_add(8)?.checked_add(len)?.checked_add(len % 2)?;
        let chunk = bytes.get(at..end.min(bytes.len()))?;
        let data = at + 8;
        match kind {
            b"VP8X" => {
                dimensions = Some((le(bytes, data + 4, 3)? + 1, le(bytes, data + 7, 3)? + 1));
            },
            b"VP8L" if dimensions.is_none() => {
                let bits = le(bytes, data + 1, 4)?;
                dimensions = Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1));
            },
            b"VP8 " if dimensions.is_none() => {
                dimensions = Some((
                    le(bytes, data + 6, 2)? & 0x3fff,
                    le(bytes, data + 8, 2)? & 0x3fff,
                ));
            },
            _ => {},
        }
        if !WEBP_DROPPED.iter().any(|dropped| kind == &dropped[..]) {
            let start = out.len();
            out.extend_from_slice(chunk);
            if kind == b"VP8X" {
                let flags = out.get_mut(start + 8)?;
                *flags &= !VP8X_METADATA_FLAGS;
            }
        }
        at = end;
    }
    let riff = u32::try_from(out.len() - 8).ok()?.to_le_bytes();
    out.get_mut(4..8)?.copy_from_slice(&riff);
    Some((out, dimensions?))
}
