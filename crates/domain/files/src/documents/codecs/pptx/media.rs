//! Uploaded pictures written into a deck's package at checkpoint.
//!
//! A live image shape names its upload by content hash. The bytes land once,
//! as `ppt/media/<hash>.<ext>`, so a later checkpoint finds them there and
//! needs no second read of the blob store. Only the picture types every
//! Office release opens are written: an image of any other type has no part,
//! and the picture step logs and skips it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::CodecError;

use super::lower::lower_deck;
use crate::documents::codecs::xml_tree::{parse_xml, splice};
use crate::documents::codecs::zip_package::Package;
use crate::documents::engine::loro::deck::{DeckModel, ShapeKind};
use crate::documents::model::Media;

const MEDIA_FOLDER: &str = "ppt/media/";
const CONTENT_TYPES: &str = "[Content_Types].xml";

fn picture_type(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    [
        (&b"\x89PNG"[..], "png", "image/png"),
        (&b"GIF8"[..], "gif", "image/gif"),
        (&[0xFF, 0xD8][..], "jpeg", "image/jpeg"),
    ]
    .into_iter()
    .find_map(|(magic, ext, mime)| bytes.starts_with(magic).then_some((ext, mime)))
}

fn part_of<'n>(names: &'n [String], hash: &str) -> Option<&'n String> {
    names.iter().find(|name| {
        name.strip_prefix(MEDIA_FOLDER)
            .and_then(|rest| rest.rsplit_once('.'))
            .is_some_and(|(stem, _)| stem == hash)
    })
}

pub fn held_images(bytes: &[u8], wanted: &[String]) -> Result<Vec<String>, CodecError> {
    let names = Package::open(bytes)?.names()?;
    Ok(wanted
        .iter()
        .filter(|hash| part_of(&names, hash).is_some())
        .cloned()
        .collect())
}

fn declare_type(package: &mut Package<'_>, ext: &str, mime: &str) -> Result<(), CodecError> {
    let types = package.require(CONTENT_TYPES)?;
    let root = parse_xml(CONTENT_TYPES, &types)?;
    let declared = root.children_named("Default").any(|entry| {
        entry
            .attr("Extension")
            .is_some_and(|held| held.eq_ignore_ascii_case(ext))
    });
    if !declared {
        let entry = format!("<Default Extension=\"{ext}\" ContentType=\"{mime}\"/>");
        package.write(
            CONTENT_TYPES,
            splice(&types, vec![(root.inner_end, root.inner_end, entry)]),
        );
    }
    Ok(())
}

pub fn lower_deck_with_media(
    bytes: &[u8],
    target: &DeckModel,
    media: &Media,
) -> Result<Vec<u8>, CodecError> {
    let mut target = target.clone();
    let mut package = Package::open(bytes)?;
    let mut names = package.names()?;
    let mut wrote = false;
    let shapes = target.slides.iter_mut().flat_map(|slide| &mut slide.shapes);
    for shape in shapes.filter(|shape| shape.kind == ShapeKind::Image) {
        let Some(src) = shape.src.clone() else {
            continue;
        };
        if let Some(part) = part_of(&names, &src) {
            shape.src = Some(part.clone());
            continue;
        }
        let Some(blob) = media.get(&src) else {
            continue;
        };
        let Some((ext, mime)) = picture_type(&blob.bytes) else {
            tracing::warn!(image = %src, "an uploaded image is not a type a deck can hold");
            continue;
        };
        let part = format!("{MEDIA_FOLDER}{src}.{ext}");
        package.write_binary(&part, blob.bytes.clone());
        declare_type(&mut package, ext, mime)?;
        names.push(part.clone());
        shape.src = Some(part);
        wrote = true;
    }
    if !wrote {
        return lower_deck(bytes, &target);
    }
    lower_deck(&package.finish()?, &target)
}
