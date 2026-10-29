use systemprompt_files::documents::codecs::docx::{
    blank_docx, import_rich, materialise_rich, materialise_rich_with,
};
use systemprompt_files::documents::model::layout::ImageWrap;
use systemprompt_files::documents::model::{BlockKind, Media, MediaBlob, ModelViewV2, RichText};

use super::docx_fixture::{entries, part};

pub(super) const BLOB: &str = "4f9c1b1c3f0e2a7d6b5c4a39281706f5e4d3c2b1a0918273645546372819a0b1";

pub(super) fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
    bytes
}

pub(super) fn rich_model() -> RichText {
    let view: ModelViewV2 = serde_json::from_value(serde_json::json!({
        "version": 2,
        "kind": "rich_text",
        "blocks": [
            {"id": "x1", "type": "heading", "level": 1, "text": "Quarterly review"},
            {"id": "x2", "type": "paragraph", "text": "Revenue grew strongly.",
             "attrs": {"align": "justify", "space_after": 6},
             "marks": [{"start": 0, "end": 7, "mark": "bold"},
                       {"start": 8, "end": 12, "mark": "color", "value": "#1F4E79"},
                       {"start": 13, "end": 21, "mark": "size", "value": "14"}]},
            {"id": "x3", "type": "list_item", "level": 0, "ordered": true, "numbering": "steps",
             "text": "Collect the figures"},
            {"id": "x4", "type": "list_item", "level": 1, "ordered": true, "numbering": "steps",
             "text": "Check them"},
            {"id": "x5", "type": "callout", "tone": "warning", "text": "Draft figures."},
            {"id": "x6", "type": "table",
             "rows": [
                [{"blocks": [{"id": "c1", "type": "paragraph", "text": "Region"}]},
                 {"blocks": []},
                 {"blocks": [{"id": "c2", "type": "paragraph", "text": "Total"}]}],
                [{"blocks": [{"id": "c3", "type": "paragraph", "text": "EMEA"}]},
                 {"blocks": [{"id": "c4", "type": "paragraph", "text": "9%"}]},
                 {"blocks": [{"id": "c5", "type": "paragraph", "text": "120"}]}],
                [{"blocks": []},
                 {"blocks": [{"id": "c6", "type": "paragraph", "text": "11%"}]},
                 {"blocks": [{"id": "c7", "type": "paragraph", "text": "140"}]}]
             ],
             "props": {
                "columns": [{"width": 120}, {"width": 80}, {"width": 80}],
                "header_rows": 1,
                "cells": [
                    [{"shade": "1F4E79", "colspan": 2}, {}, {"shade": "1F4E79"}],
                    [{"rowspan": 2, "valign": "center"}, {}, {"align": "right"}]
                ]
             }},
            {"id": "x7", "type": "image", "blob": BLOB, "alt": "Chart", "width": 320,
             "height": 160, "caption": "Figure 1", "wrap": "square"},
            {"id": "x8", "type": "toc", "levels": 2}
        ],
        "document": {
            "numbering": [{"id": "steps", "levels": [
                {"format": "decimal", "text": "%1.", "start": 1, "indent": 36},
                {"format": "lower_letter", "text": "%2)", "start": 1, "indent": 72}
            ]}],
            "section": {
                "size": {"width": 595, "height": 842},
                "margins": {"top": 72, "right": 72, "bottom": 72, "left": 72},
                "header": {"blocks": [{"id": "h1", "type": "paragraph", "text": "Confidential"}]},
                "footer": {"blocks": [{"id": "f1", "type": "paragraph", "text": "Page 1",
                    "attrs": {"align": "center"},
                    "marks": [{"start": 5, "end": 6, "mark": "field", "code": "PAGE"}]}]}
            }
        }
    }))
    .expect("a valid v2 model");
    view.into_model().expect("a supported version")
}

pub(super) fn media() -> Media {
    let mut media = Media::new();
    media.insert(
        BLOB.to_owned(),
        MediaBlob {
            bytes: png(640, 320),
            mime: "image/png".to_owned(),
        },
    );
    media
}

pub(super) fn written() -> Vec<u8> {
    let base = blank_docx().expect("blank");
    materialise_rich_with(&base, &rich_model(), &media()).expect("materialise")
}

#[test]
fn a_rich_docx_imports_materialises_and_imports_to_the_same_model() {
    let first = written();
    let model = import_rich(&first).expect("import");
    let again = materialise_rich(&first, &model).expect("materialise");
    assert_eq!(import_rich(&again).expect("reimport"), model);
}

#[test]
fn an_unchanged_rich_model_leaves_every_part_byte_identical() {
    let first = written();
    let model = import_rich(&first).expect("import");
    let again = materialise_rich(&first, &model).expect("materialise");
    assert_eq!(entries(&again), entries(&first));
}

#[test]
fn tables_keep_their_merges_shading_widths_and_header_rows() {
    let model = import_rich(&written()).expect("import");
    let (rows, props) = model
        .blocks
        .iter()
        .find_map(|block| match &block.kind {
            BlockKind::Table { rows, props } => Some((rows, props)),
            _ => None,
        })
        .expect("a table");
    assert_eq!(rows.len(), 3);
    assert_eq!(props.header_rows, 1);
    assert_eq!(
        props.columns.iter().map(|c| c.width).collect::<Vec<_>>(),
        vec![Some(120), Some(80), Some(80)]
    );
    assert_eq!(props.cell(0, 0).colspan(), 2);
    assert_eq!(props.cell(0, 0).shade.as_deref(), Some("1F4E79"));
    assert_eq!(props.cell(1, 0).rowspan(), 2);
    assert!(
        rows[2][0].blocks.is_empty(),
        "the covered cell keeps its slot"
    );
    let xml = part(&written(), "word/document.xml");
    assert!(xml.contains("<w:gridSpan w:val=\"2\"/>"));
    assert!(xml.contains("<w:vMerge w:val=\"restart\"/>"));
    assert!(xml.contains("<w:tblHeader/>"));
}

#[test]
fn numbering_sections_images_and_fields_survive_the_round_trip() {
    let bytes = written();
    let model = import_rich(&bytes).expect("import");
    let settings = model.settings();
    let steps = settings.numbering("steps").expect("the steps definition");
    assert_eq!(steps.levels.len(), 2);
    let item = model
        .blocks
        .iter()
        .find(|block| block.text == "Check them")
        .expect("list item");
    assert!(matches!(
        &item.kind,
        BlockKind::ListItem { level: 1, ordered: true, numbering: Some(id), .. } if id == "steps"
    ));
    let section = settings.section.expect("final section");
    assert_eq!(section.size.map(|s| (s.width, s.height)), Some((595, 842)));
    let footer = section.footer.expect("footer");
    assert_eq!(footer.blocks[0].text, "Page 1");
    assert!(
        footer.blocks[0]
            .marks
            .iter()
            .any(|m| m.kind.key() == "field")
    );
    assert_eq!(
        section.header.expect("header").blocks[0].text,
        "Confidential"
    );
    let (blob, width, caption, wrap) = model
        .blocks
        .iter()
        .find_map(|block| match &block.kind {
            BlockKind::Image {
                blob,
                width,
                caption,
                wrap,
                ..
            } => Some((blob, width, caption, wrap)),
            _ => None,
        })
        .expect("an image");
    assert!(blob.starts_with("docx-media:media/"));
    assert_eq!(*width, Some(320));
    assert_eq!(caption.as_deref(), Some("Figure 1"));
    assert_eq!(*wrap, ImageWrap::Square);
    assert!(
        model
            .blocks
            .iter()
            .any(|block| matches!(block.kind, BlockKind::Toc { levels: 2 }))
    );
    assert!(part(&bytes, "word/numbering.xml").contains("<w:name w:val=\"steps\"/>"));
    assert!(part(&bytes, "word/settings.xml").contains("<w:updateFields"));
}

#[test]
fn an_image_without_uploaded_bytes_is_refused_by_name() {
    let base = blank_docx().expect("blank");
    let error = materialise_rich(&base, &rich_model()).expect_err("refused");
    assert!(error.to_string().contains(BLOB));
}
