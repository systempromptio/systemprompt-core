//! An image block written as a `w:drawing`, with its picture part.
//!
//! An image the document already holds (`docx-media:<target>`) is written
//! against that part; any other blob must be in the materialise's media,
//! and is added as `word/media/sp-<ref>.<ext>` with its content type and an
//! image relationship. A size the model leaves out comes from the picture's
//! own pixel size (scaled to keep its aspect ratio), capped at the text
//! width. An inline image is a `wp:inline`; any other wrap a `wp:anchor`
//! at the paragraph. A caption is the next paragraph, styled `Caption`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::markup::run;
use super::parts::read_part;
use super::read_drawing::{
    EMU_PER_PX, MAX_PART_BYTES, MEDIA_SCHEME, image_type, pixel_size, saved_upload_part,
};
use super::write::{WriteCtx, escape_attr, ppr};
use crate::documents::model::layout::ImageWrap;
use crate::documents::model::{Block, BlockKind};

const WP_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const PIC_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
const MAX_WIDTH: u64 = 624;
const MAX_PIXELS: u64 = 4096;

pub(super) fn image_block(block: &Block, ctx: &mut WriteCtx<'_>) -> String {
    let BlockKind::Image {
        blob,
        alt,
        width,
        height,
        caption,
        wrap,
    } = &block.kind
    else {
        return String::new();
    };
    if ctx.in_part {
        ctx.fail("images in headers and footers are not supported yet");
        return String::new();
    }
    let Some((target, natural)) = picture(blob, ctx) else {
        return String::new();
    };
    let Some(rid) = ctx.links.part_id("image", &target) else {
        ctx.fail("the document has no relationships part to hold an image");
        return String::new();
    };
    let (cx, cy) = size(*width, *height, natural);
    ctx.drawing_id = ctx.drawing_id.saturating_add(1);
    let drawing = drawing(&rid, ctx.drawing_id, alt, (cx, cy), *wrap);
    let props = ppr(&block.attrs, None, None, "", None);
    let mut out = format!("<w:p>{props}<w:r><w:drawing>{drawing}</w:drawing></w:r></w:p>");
    if let Some(caption) = caption {
        out.push_str(&format!(
            "<w:p><w:pPr><w:pStyle w:val=\"Caption\"/></w:pPr>{}</w:p>",
            run(None, caption)
        ));
    }
    out
}

fn picture(blob: &str, ctx: &mut WriteCtx<'_>) -> Option<(String, Option<(u32, u32)>)> {
    if let Some(target) = blob.strip_prefix(MEDIA_SCHEME) {
        if !media_target_ok(target) {
            ctx.fail(format!(
                "image {blob} does not name a picture part under media/"
            ));
            return None;
        }
        let natural = read_part(ctx.base, &format!("word/{target}"))
            .ok()
            .flatten()
            .and_then(|bytes| pixel_size(&bytes));
        return Some((target.to_owned(), natural));
    }
    let resolved = ctx.media;
    if !resolved.contains_key(blob) {
        let base = ctx.base;
        let saved = ctx
            .saved_uploads
            .entry(blob.to_owned())
            .or_insert_with(|| {
                saved_upload_part(base, blob, MAX_PART_BYTES)
                    .map(|(target, bytes)| (target, pixel_size(&bytes)))
            })
            .clone();
        if saved.is_some() {
            return saved;
        }
    }
    let Some(media) = resolved.get(blob) else {
        ctx.fail(format!(
            "image {blob} was not resolved; upload it and pass its blob_ref"
        ));
        return None;
    };
    let Some((extension, mime)) = image_type(&media.bytes) else {
        ctx.fail(format!("image {blob} is not a PNG, JPEG or GIF"));
        return None;
    };
    let stem: String = blob.chars().filter(char::is_ascii_alphanumeric).collect();
    let target = format!("media/sp-{stem}.{extension}");
    let part = format!("word/{target}");
    let in_base = read_part(ctx.base, &part).ok().flatten().is_some();
    if !in_base && ctx.edits.get(&part).is_none() {
        ctx.edits.put(&part, media.bytes.clone());
        ctx.edits.default_type(extension, mime);
    }
    Some((target, pixel_size(&media.bytes)))
}

fn size(width: Option<u32>, height: Option<u32>, natural: Option<(u32, u32)>) -> (u64, u64) {
    let (nw, nh) = natural
        .filter(|(w, h)| *w > 0 && *h > 0)
        .map_or((480, 320), |(w, h)| (u64::from(w), u64::from(h)));
    let scale = |a: u64, b: u64, c: u64| a.saturating_mul(b) / c.max(1);
    let (w, h) = match (width.map(u64::from), height.map(u64::from)) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, scale(w, nh, nw)),
        (None, Some(h)) => (scale(h, nw, nh), h),
        (None, None) if nw > MAX_WIDTH => (MAX_WIDTH, scale(MAX_WIDTH, nh, nw)),
        (None, None) => (nw, nh),
    };
    let emu = |px: u64| px.clamp(1, MAX_PIXELS).saturating_mul(EMU_PER_PX);
    (emu(w), emu(h))
}

fn drawing(rid: &str, id: u32, alt: &str, (cx, cy): (u64, u64), wrap: ImageWrap) -> String {
    let alt = escape_attr(alt);
    let graphic = format!(
        "<wp:cNvGraphicFramePr><a:graphicFrameLocks xmlns:a=\"{A_NS}\" noChangeAspect=\"1\"/>\
         </wp:cNvGraphicFramePr><a:graphic xmlns:a=\"{A_NS}\"><a:graphicData uri=\"{PIC_NS}\">\
         <pic:pic xmlns:pic=\"{PIC_NS}\"><pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"Picture \
         {id}\" descr=\"{alt}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip \
         r:embed=\"{rid}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr>\
         <a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom \
         prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic>"
    );
    let doc_pr = format!("<wp:docPr id=\"{id}\" name=\"Picture {id}\" descr=\"{alt}\"/>");
    let extent = format!("<wp:extent cx=\"{cx}\" cy=\"{cy}\"/>");
    if wrap == ImageWrap::Inline {
        return format!(
            "<wp:inline xmlns:wp=\"{WP_NS}\" distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">\
             {extent}<wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>{doc_pr}{graphic}\
             </wp:inline>"
        );
    }
    let (wrap_xml, behind) = match wrap {
        ImageWrap::Square => ("<wp:wrapSquare wrapText=\"bothSides\"/>", "0"),
        ImageWrap::TopBottom => ("<wp:wrapTopAndBottom/>", "0"),
        ImageWrap::Behind => ("<wp:wrapNone/>", "1"),
        ImageWrap::InFront | ImageWrap::Inline => ("<wp:wrapNone/>", "0"),
    };
    format!(
        "<wp:anchor xmlns:wp=\"{WP_NS}\" distT=\"0\" distB=\"0\" distL=\"114300\" \
         distR=\"114300\" simplePos=\"0\" relativeHeight=\"{id}\" behindDoc=\"{behind}\" \
         locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/>\
         <wp:positionH relativeFrom=\"column\"><wp:posOffset>0</wp:posOffset></wp:positionH>\
         <wp:positionV relativeFrom=\"paragraph\"><wp:posOffset>0</wp:posOffset></wp:positionV>\
         {extent}<wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>{wrap_xml}{doc_pr}{graphic}\
         </wp:anchor>"
    )
}

pub(super) fn max_drawing_id(xml: &str) -> u32 {
    ["<wp:docPr", "<pic:cNvPr"]
        .iter()
        .flat_map(|tag| xml.match_indices(tag).map(|(at, _)| &xml[at..]))
        .filter_map(|from| {
            let tag = from.split('>').next()?;
            let value = tag.split(" id=\"").nth(1)?.split('"').next()?;
            value.parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0)
}

fn media_target_ok(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    let image = [
        ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".tif", ".tiff", ".emf", ".wmf",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext));
    lower.starts_with("media/") && !lower.contains("..") && image
}
