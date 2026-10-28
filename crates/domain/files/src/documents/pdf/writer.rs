//! Serialises laid-out pages as a PDF 1.7 file: one Flate-compressed
//! content stream per page, the four standard fonts shared by every page,
//! and an info dictionary with a fixed creation date. Object numbers are
//! assigned in a fixed order, so the same pages produce the same bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::Write;

use chrono::{DateTime, Datelike, Timelike, Utc};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use pdf_writer::{Content, Date, Filter, Finish, Pdf, Rect, Ref, Str, TextStr};

use super::fonts::{Font, encode};
use super::layout::{PAGE_HEIGHT, PAGE_WIDTH, Page};

const PRODUCER: &str = "systemprompt";
const RULE_WIDTH: f32 = 0.5;
const RULE_GRAY: f32 = 0.4;

#[derive(Debug, Clone, Copy)]
pub(super) struct Metadata<'a> {
    pub(super) title: &'a str,
    pub(super) subject: &'a str,
    pub(super) created_at: DateTime<Utc>,
}

pub(super) fn write(pages: &[Page], metadata: Metadata<'_>) -> Result<Vec<u8>, std::io::Error> {
    let mut pdf = Pdf::new();
    let catalog = Ref::new(1);
    let tree = Ref::new(2);
    let info = Ref::new(3);
    let fonts: Vec<(Font, Ref)> = Font::ALL
        .iter()
        .enumerate()
        .map(|(index, font)| (*font, Ref::new(4 + ref_index(index))))
        .collect();
    let first_page = 4 + ref_index(fonts.len());
    let page_refs: Vec<Ref> = (0..pages.len())
        .map(|index| Ref::new(first_page + 2 * ref_index(index)))
        .collect();

    pdf.catalog(catalog).pages(tree);
    pdf.pages(tree)
        .kids(page_refs.iter().copied())
        .count(i32::try_from(pages.len()).unwrap_or(i32::MAX));
    pdf.document_info(info)
        .title(TextStr(metadata.title))
        .subject(TextStr(metadata.subject))
        .producer(TextStr(PRODUCER))
        .creator(TextStr(PRODUCER))
        .creation_date(pdf_date(metadata.created_at))
        .modified_date(pdf_date(metadata.created_at));
    for (font, id) in &fonts {
        pdf.type1_font(*id)
            .base_font(font.base_font())
            .encoding_predefined(pdf_writer::Name(b"WinAnsiEncoding"));
    }
    for (page, id) in pages.iter().zip(&page_refs) {
        let content_id = Ref::new(id.get() + 1);
        let mut node = pdf.page(*id);
        node.media_box(Rect::new(0.0, 0.0, PAGE_WIDTH, PAGE_HEIGHT));
        node.parent(tree);
        node.contents(content_id);
        let mut resources = node.resources();
        let mut font_dict = resources.fonts();
        for (font, font_id) in &fonts {
            font_dict.pair(font.resource(), *font_id);
        }
        font_dict.finish();
        resources.finish();
        node.finish();
        let compressed = compress(&content_of(page))?;
        pdf.stream(content_id, &compressed)
            .filter(Filter::FlateDecode);
    }
    Ok(pdf.finish())
}

fn ref_index(index: usize) -> i32 {
    i32::try_from(index).unwrap_or(i32::MAX)
}

fn content_of(page: &Page) -> Vec<u8> {
    let mut content = Content::new();
    content
        .set_line_width(RULE_WIDTH)
        .set_stroke_gray(RULE_GRAY);
    for rule in &page.rules {
        content
            .move_to(rule.x1, rule.y1)
            .line_to(rule.x2, rule.y2)
            .stroke();
    }
    for text in &page.texts {
        let bytes = encode(&text.content);
        content
            .begin_text()
            .set_font(text.font.resource(), text.size)
            .next_line(text.x, text.y)
            .show(Str(&bytes))
            .end_text();
    }
    content.finish().into_vec()
}

fn compress(bytes: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes)?;
    encoder.finish()
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
