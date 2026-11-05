//! Decodes an image the revision holds into what a PDF image object takes.
//! A JPEG passes through untouched; a PNG is decoded and deflated, its alpha
//! kept as a soft mask. Any other format is not drawn, and the caller shows
//! the image's alt text instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{Cursor, Write};

use flate2::Compression;
use flate2::write::ZlibEncoder;
use png::{ColorType, Decoder, Transformations};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Encoding {
    Jpeg,
    Flate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Picture {
    pub(super) encoding: Encoding,
    pub(super) data: Vec<u8>,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) gray: bool,
    pub(super) alpha: Option<Vec<u8>>,
}

#[must_use]
pub(super) fn decode(bytes: &[u8]) -> Option<Picture> {
    if bytes.starts_with(b"\x89PNG") {
        png_picture(bytes)
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        jpeg_picture(bytes)
    } else {
        None
    }
}

fn deflate(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).ok()?;
    encoder.finish().ok()
}

fn png_picture(bytes: &[u8]) -> Option<Picture> {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buffer).ok()?;
    let pixels = buffer.get(..frame.buffer_size())?;
    let (channels, gray, has_alpha) = match frame.color_type {
        ColorType::Grayscale => (1, true, false),
        ColorType::GrayscaleAlpha => (2, true, true),
        ColorType::Rgb => (3, false, false),
        ColorType::Rgba => (4, false, true),
        ColorType::Indexed => return None,
    };
    let colour_channels = if has_alpha { channels - 1 } else { channels };
    let mut colour = Vec::with_capacity(pixels.len());
    let mut alpha = Vec::new();
    for pixel in pixels.chunks_exact(channels) {
        colour.extend_from_slice(&pixel[..colour_channels]);
        if has_alpha {
            alpha.push(pixel[channels - 1]);
        }
    }
    Some(Picture {
        encoding: Encoding::Flate,
        data: deflate(&colour)?,
        width: frame.width,
        height: frame.height,
        gray,
        alpha: if has_alpha { deflate(&alpha) } else { None },
    })
}

const fn is_frame_header(marker: u8) -> bool {
    matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc)
}

fn jpeg_picture(bytes: &[u8]) -> Option<Picture> {
    let mut at = 2;
    loop {
        if *bytes.get(at)? != 0xff {
            return None;
        }
        let marker = *bytes.get(at + 1)?;
        if marker == 0xff {
            at += 1;
            continue;
        }
        let length = usize::from(u16::from_be_bytes([
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ]));
        if is_frame_header(marker) {
            let height = u16::from_be_bytes([*bytes.get(at + 5)?, *bytes.get(at + 6)?]);
            let width = u16::from_be_bytes([*bytes.get(at + 7)?, *bytes.get(at + 8)?]);
            let gray = match *bytes.get(at + 9)? {
                1 => true,
                3 => false,
                _ => return None,
            };
            return Some(Picture {
                encoding: Encoding::Jpeg,
                data: bytes.to_vec(),
                width: u32::from(width),
                height: u32::from(height),
                gray,
                alpha: None,
            });
        }
        at += 2 + length;
    }
}
