//! Serialises laid-out pages as a PDF 1.7 file: one Flate-compressed content
//! stream per page, a subset of each face the text uses, the images, link
//! annotations and an info dictionary with a fixed creation date. Object
//! numbers are assigned in a fixed order, so the same pages produce the same
//! bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Datelike, Timelike, Utc};
use pdf_writer::types::{ActionType, AnnotationType};
use pdf_writer::{Content, Date, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

use super::compress;
use super::font_embed::{FontRefs, Subset, embed};
use super::fonts::Font;
use super::images::{Encoding, Picture};
use super::layout_model::{PAGE_HEIGHT, PAGE_WIDTH, Page};
use super::runs::Rgb;

const PRODUCER: &str = "systemprompt";

#[derive(Debug, Clone, Copy)]
pub(super) struct Metadata<'a> {
    pub(super) title: &'a str,
    pub(super) subject: &'a str,
    pub(super) created_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
struct Refs(i32);

impl Refs {
    fn next(&mut self) -> Ref {
        self.0 += 1;
        Ref::new(self.0)
    }
}

fn used_chars(pages: &[Page]) -> BTreeMap<Font, BTreeSet<char>> {
    let mut used: BTreeMap<Font, BTreeSet<char>> = BTreeMap::new();
    for text in pages.iter().flat_map(|page| &page.texts) {
        used.entry(text.font)
            .or_default()
            .extend(text.content.chars());
    }
    used
}

fn write_picture(
    pdf: &mut Pdf,
    picture: &Picture,
    id: Ref,
    mask: Option<Ref>,
) -> Result<(), std::io::Error> {
    let width = i32::try_from(picture.width).unwrap_or(i32::MAX);
    let height = i32::try_from(picture.height).unwrap_or(i32::MAX);
    if let (Some(alpha), Some(mask)) = (&picture.alpha, mask) {
        let mut image = pdf.image_xobject(mask, alpha);
        image.filter(Filter::FlateDecode);
        image.width(width).height(height);
        image.color_space().device_gray();
        image.bits_per_component(8);
        image.finish();
    }
    let mut image = pdf.image_xobject(id, &picture.data);
    image.filter(match picture.encoding {
        Encoding::Jpeg => Filter::DctDecode,
        Encoding::Flate => Filter::FlateDecode,
    });
    image.width(width).height(height);
    if picture.gray {
        image.color_space().device_gray();
    } else {
        image.color_space().device_rgb();
    }
    image.bits_per_component(8);
    if let Some(mask) = mask {
        image.s_mask(mask);
    }
    image.finish();
    Ok(())
}

fn picture_name(index: usize) -> String {
    format!("Im{index}")
}

pub(super) fn write(
    pages: &[Page],
    pictures: &[Picture],
    metadata: Metadata<'_>,
) -> Result<Vec<u8>, std::io::Error> {
    let mut pdf = Pdf::new();
    let mut refs = Refs::default();
    let (catalog, tree, info) = (refs.next(), refs.next(), refs.next());

    pdf.document_info(info)
        .title(TextStr(metadata.title))
        .subject(TextStr(metadata.subject))
        .producer(TextStr(PRODUCER))
        .creator(TextStr(PRODUCER))
        .creation_date(pdf_date(metadata.created_at))
        .modified_date(pdf_date(metadata.created_at));

    let mut fonts: Vec<(Font, Ref, Subset)> = Vec::new();
    for (font, used) in used_chars(pages) {
        let font_refs = FontRefs {
            type0: refs.next(),
            cid: refs.next(),
            descriptor: refs.next(),
            file: refs.next(),
            cmap: refs.next(),
        };
        let subset = embed(&mut pdf, font, &used, font_refs)?;
        fonts.push((font, font_refs.type0, subset));
    }

    let mut picture_refs = Vec::with_capacity(pictures.len());
    for picture in pictures {
        let id = refs.next();
        let mask = picture.alpha.as_ref().map(|_| refs.next());
        write_picture(&mut pdf, picture, id, mask)?;
        picture_refs.push(id);
    }

    let page_refs: Vec<(Ref, Ref)> = pages.iter().map(|_| (refs.next(), refs.next())).collect();
    pdf.catalog(catalog).pages(tree);
    pdf.pages(tree)
        .kids(page_refs.iter().map(|(page, _)| *page))
        .count(i32::try_from(pages.len()).unwrap_or(i32::MAX));

    let subsets: BTreeMap<Font, &Subset> = fonts.iter().map(|(f, _, s)| (*f, s)).collect();
    for (page, (id, content_id)) in pages.iter().zip(&page_refs) {
        let links: Vec<Ref> = page.links.iter().map(|_| refs.next()).collect();
        write_page(
            &mut pdf,
            page,
            (*id, *content_id, tree),
            (&fonts, &picture_refs),
            &links,
        );
        let compressed = compress(&content_of(page, &subsets))?;
        pdf.stream(*content_id, &compressed)
            .filter(Filter::FlateDecode);
    }
    Ok(pdf.finish())
}

fn write_page(
    pdf: &mut Pdf,
    page: &Page,
    (id, content_id, tree): (Ref, Ref, Ref),
    (fonts, pictures): (&[(Font, Ref, Subset)], &[Ref]),
    links: &[Ref],
) {
    let mut node = pdf.page(id);
    node.media_box(Rect::new(0.0, 0.0, PAGE_WIDTH, PAGE_HEIGHT));
    node.parent(tree);
    node.contents(content_id);
    let mut resources = node.resources();
    let mut font_dict = resources.fonts();
    for (font, font_id, _) in fonts {
        font_dict.pair(Name(font.resource().as_bytes()), *font_id);
    }
    font_dict.finish();
    let mut objects = resources.x_objects();
    for placed in &page.pictures {
        if let Some(picture) = pictures.get(placed.picture) {
            let name = picture_name(placed.picture);
            objects.pair(Name(name.as_bytes()), *picture);
        }
    }
    objects.finish();
    resources.finish();
    if !links.is_empty() {
        node.annotations(links.iter().copied());
    }
    node.finish();
    for (link, id) in page.links.iter().zip(links) {
        let r = link.rect;
        let mut annotation = pdf.annotation(*id);
        annotation
            .subtype(AnnotationType::Link)
            .rect(Rect::new(r.x, r.y, r.x + r.w, r.y + r.h))
            .border(0.0, 0.0, 0.0, None);
        annotation
            .action()
            .action_type(ActionType::Uri)
            .uri(Str(link.href.as_bytes()));
    }
}

fn to_unit(color: Rgb) -> (f32, f32, f32) {
    let unit = |value: u8| f32::from(value) / 255.0;
    (unit(color[0]), unit(color[1]), unit(color[2]))
}

fn content_of(page: &Page, subsets: &BTreeMap<Font, &Subset>) -> Vec<u8> {
    let mut content = Content::new();
    for fill in &page.fills {
        let (r, g, b) = to_unit(fill.color);
        content
            .set_fill_rgb(r, g, b)
            .rect(fill.rect.x, fill.rect.y, fill.rect.w, fill.rect.h)
            .fill_nonzero();
    }
    for placed in &page.pictures {
        let r = placed.rect;
        let name = picture_name(placed.picture);
        content
            .save_state()
            .transform([r.w, 0.0, 0.0, r.h, r.x, r.y])
            .x_object(Name(name.as_bytes()))
            .restore_state();
    }
    for rule in &page.rules {
        let (r, g, b) = to_unit(rule.color);
        content
            .set_line_width(rule.width)
            .set_stroke_rgb(r, g, b)
            .move_to(rule.x1, rule.y1)
            .line_to(rule.x2, rule.y2)
            .stroke();
    }
    for text in &page.texts {
        let Some(subset) = subsets.get(&text.font) else {
            continue;
        };
        let (r, g, b) = to_unit(text.color);
        let bytes = subset.encode(text.font, &text.content);
        content
            .set_fill_rgb(r, g, b)
            .begin_text()
            .set_font(Name(text.font.resource().as_bytes()), text.size)
            .next_line(text.x, text.y)
            .show(Str(&bytes))
            .end_text();
    }
    content.finish().into_vec()
}

fn pdf_date(at: DateTime<Utc>) -> Date {
    Date::new(u16::try_from(at.year()).unwrap_or(1970))
        .month(u8::try_from(at.month()).unwrap_or(1))
        .day(u8::try_from(at.day()).unwrap_or(1))
        .hour(u8::try_from(at.hour()).unwrap_or(0))
        .minute(u8::try_from(at.minute()).unwrap_or(0))
        .second(u8::try_from(at.second()).unwrap_or(0))
        .utc_offset_hour(0)
        .utc_offset_minute(0)
}
