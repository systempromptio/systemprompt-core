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
use super::read_drawing::{EMU_PER_PX, MEDIA_SCHEME, image_type, pixel_size};
use super::write::{WriteCtx, escape_attr, ppr};
use crate::documents::model::{Block, BlockKind};
use crate::documents::model::layout::ImageWrap;

const WP_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const PIC_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
const MAX_WIDTH: u32 = 624;

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
    ctx.drawing_id += 1;
    let drawing = drawing(&rid, ctx.drawing_id, alt, cx, cy, *wrap);
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

/// The picture's target relative to `word/`, and its pixel size.
fn picture(blob: &str, ctx: &mut WriteCtx<'_>) -> Option<(String, Option<(u32, u32)>)> {
    if let Some(target) = blob.strip_prefix(MEDIA_SCHEME) {
        let natural = read_part(ctx.base, &format!("word/{target}"))
            .ok()
            .flatten()
            .and_then(|bytes| pixel_size(&bytes));
        return Some((target.to_owned(), natural));
    }
    let resolved = ctx.media;
    let Some(media) = resolved.get(blob) else {
        ctx.fail(format!("image {blob} was not resolved; upload it and pass its blob_ref"));
        return None;
    };
    let Some((extension, mime)) = image_type(&media.bytes) else {
        ctx.fail(format!("image {blob} is not a PNG, JPEG or GIF"));
        return None;
    };
    let stem: String = blob
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .chars()
        .rev()
        .take(24)
        .collect();
    let target = format!("media/sp-{stem}.{extension}");
    let part = format!("word/{target}");
    if ctx.edits.get(&part).is_none() {
        ctx.edits.put(&part, media.bytes.clone());
        ctx.edits.default_type(extension, mime);
    }
    Some((target, pixel_size(&media.bytes)))
}

fn size(width: Option<u32>, height: Option<u32>, natural: Option<(u32, u32)>) -> (u64, u64) {
    let (nw, nh) = natural.filter(|(w, h)| *w > 0 && *h > 0).unwrap_or((480, 320));
    let (w, h) = match (width, height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, w * nh / nw),
        (None, Some(h)) => (h * nw / nh, h),
        (None, None) if nw > MAX_WIDTH => (MAX_WIDTH, MAX_WIDTH * nh / nw),
        (None, None) => (nw, nh),
    };
    (u64::from(w.max(1)) * EMU_PER_PX, u64::from(h.max(1)) * EMU_PER_PX)
}

fn drawing(rid: &str, id: u32, alt: &str, cx: u64, cy: u64, wrap: ImageWrap) -> String {
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
