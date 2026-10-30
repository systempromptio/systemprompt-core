//! Adds an image shape to a slide as a `p:pic` over a picture part the
//! package already holds.
//!
//! An added image names its part (`ppt/media/image1.png`) in its `src`. A
//! part the package does not hold, or an image with no frame, is
//! logged and skipped; nothing is uploaded here. The slide's relationships
//! gain an image relationship to the part under the next free `rId`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::CodecError;

use super::package::{relationships, rels_part};
use super::structure::xfrm_xml;
use crate::documents::codecs::zip_package::Package;
use crate::documents::engine::loro::deck::DeckShape;

const IMAGE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const EMPTY_RELS: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n",
    "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">",
    "</Relationships>"
);

fn relative(from_part: &str, to_part: &str) -> String {
    let from: Vec<&str> = from_part.split('/').collect();
    let to: Vec<&str> = to_part.split('/').collect();
    let dirs = &from[..from.len().saturating_sub(1)];
    let common = dirs.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; dirs.len() - common];
    parts.extend(&to[common..]);
    parts.join("/")
}

fn link(package: &mut Package<'_>, slide: &str, media: &str) -> Result<String, CodecError> {
    let (xml, rels) = relationships(package, slide)?;
    if let Some(rel) = rels
        .iter()
        .find(|rel| rel.rel_type == IMAGE_REL && rel.target == media)
    {
        return Ok(rel.id.clone());
    }
    let next = rels
        .iter()
        .filter_map(|rel| rel.id.strip_prefix("rId")?.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let id = format!("rId{next}");
    let xml = if xml.is_empty() {
        EMPTY_RELS.to_owned()
    } else {
        xml
    };
    let entry = format!(
        "<Relationship Id=\"{id}\" Type=\"{IMAGE_REL}\" Target=\"{}\"/>",
        escape(relative(slide, media).as_str())
    );
    let Some(at) = xml.rfind("</Relationships>") else {
        return Err(CodecError::Malformed {
            detail: "a slide's relationships part has no closing tag",
        });
    };
    package.write(
        &rels_part(slide),
        format!("{}{entry}{}", &xml[..at], &xml[at..]),
    );
    Ok(id)
}

/// The `p:pic` for an added image shape, or `None` when it was skipped.
pub(super) fn picture_xml(
    package: &mut Package<'_>,
    slide: &str,
    id: u64,
    shape: &DeckShape,
) -> Result<Option<String>, CodecError> {
    let Some(media) = shape.src.as_deref() else {
        tracing::warn!(slide, shape = %shape.id, "an added image names no picture part");
        return Ok(None);
    };
    if !package.names()?.iter().any(|name| name == media) {
        tracing::warn!(
            slide,
            media,
            "an added image's picture part is not in the package"
        );
        return Ok(None);
    }
    let Some(frame) = shape.frame else {
        tracing::warn!(slide, media, "an added image has no frame");
        return Ok(None);
    };
    let rel = link(package, slide, media)?;
    Ok(Some(format!(
        "<p:pic><p:nvPicPr><p:cNvPr id=\"{id}\" name=\"Picture {id}\"/><p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed=\"{rel}\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>",
        xfrm_xml(frame)
    )))
}
