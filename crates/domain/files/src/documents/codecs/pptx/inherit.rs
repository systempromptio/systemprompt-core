//! Placeholder frames a slide inherits from its layout and master.
//!
//! A placeholder with no `a:xfrm` of its own takes the frame of the layout
//! placeholder it matches, and a layout placeholder with none takes the
//! master's. A slide placeholder matches a layout placeholder by `idx` when
//! both carry one, otherwise by type; a layout placeholder matches the
//! master's by type. Types are compared by family: `title`/`ctrTitle` are a
//! title, everything else that holds text (`body`, `obj`, `subTitle`, an
//! untyped placeholder) is a body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::CodecError;

use super::package::relationships;
use super::projection::{shape_elements, sp_tree};
use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::codecs::zip_package::Package;

pub(super) type Rect = (i64, i64, i64, i64);

#[derive(Debug, Clone)]
#[cfg_attr(
    not(feature = "collab-loro"),
    expect(dead_code, reason = "only the live deck model reads inherited frames")
)]
pub(super) struct PlaceholderFrame {
    pub family: &'static str,
    pub idx: Option<String>,
    pub rect: Rect,
}

fn family(ph_type: Option<&str>) -> &'static str {
    match ph_type {
        Some("title" | "ctrTitle") => "title",
        Some("dt") => "dt",
        Some("ftr") => "ftr",
        Some("sldNum") => "sldNum",
        Some("hdr") => "hdr",
        Some("pic") => "pic",
        _ => "body",
    }
}

fn ph(element: &Element) -> Option<&Element> {
    element
        .child("p:nvSpPr")
        .or_else(|| element.child("p:nvPicPr"))
        .or_else(|| element.child("p:nvGraphicFramePr"))?
        .child("p:nvPr")?
        .child("p:ph")
}

fn own_rect(element: &Element) -> Option<Rect> {
    let xfrm = element
        .child("p:spPr")
        .and_then(|pr| pr.child("a:xfrm"))
        .or_else(|| element.child("p:xfrm"))?;
    let off = xfrm.child("a:off")?;
    let ext = xfrm.child("a:ext")?;
    let number = |el: &Element, key: &str| el.attr(key)?.parse::<i64>().ok();
    Some((
        number(off, "x")?,
        number(off, "y")?,
        number(ext, "cx")?,
        number(ext, "cy")?,
    ))
}

struct Declared {
    family: &'static str,
    idx: Option<String>,
    rect: Option<Rect>,
}

fn declared(package: &mut Package<'_>, part: &str) -> Result<Vec<Declared>, CodecError> {
    let xml = package.require(part)?;
    let root = parse_xml(part, &xml)?;
    Ok(sp_tree(&root).map_or_else(Vec::new, |tree| {
        shape_elements(tree)
            .filter_map(|element| {
                let ph = ph(element)?;
                Some(Declared {
                    family: family(ph.attr("type")),
                    idx: ph.attr("idx").map(str::to_owned),
                    rect: own_rect(element),
                })
            })
            .collect()
    }))
}

fn master_part(package: &mut Package<'_>, layout: &str) -> Result<Option<String>, CodecError> {
    let (_, rels) = relationships(package, layout)?;
    Ok(rels
        .into_iter()
        .find(|rel| rel.rel_type.ends_with("/slideMaster"))
        .map(|rel| rel.target))
}

pub(super) fn layout_frames(
    package: &mut Package<'_>,
    layout: &str,
) -> Result<Vec<PlaceholderFrame>, CodecError> {
    let layout_shapes = declared(package, layout)?;
    let master = match master_part(package, layout)? {
        Some(master) => declared(package, &master)?,
        None => Vec::new(),
    };
    Ok(layout_shapes
        .into_iter()
        .filter_map(|shape| {
            let rect = shape.rect.or_else(|| {
                master
                    .iter()
                    .find(|m| m.family == shape.family)
                    .and_then(|m| m.rect)
            })?;
            Some(PlaceholderFrame {
                family: shape.family,
                idx: shape.idx,
                rect,
            })
        })
        .collect())
}

#[cfg(feature = "collab-loro")]
pub(super) fn inherited(frames: &[PlaceholderFrame], element: &Element) -> Option<Rect> {
    let ph = ph(element)?;
    let idx = ph.attr("idx");
    let by_idx = idx.and_then(|idx| frames.iter().find(|f| f.idx.as_deref() == Some(idx)));
    let wanted = family(ph.attr("type"));
    by_idx
        .or_else(|| frames.iter().find(|f| f.family == wanted))
        .map(|f| f.rect)
}
