use serde_json::json;
use systemprompt_files::documents::codecs::MarkdownCodec;
use systemprompt_files::documents::model::ops::apply_rich_ops;
use systemprompt_provider_contracts::documents::{
    ApplyTheme, CodecError, DocumentCodec, DocumentFormat, EditOp, SetSection, SetStyle,
};

use super::SAMPLE_MD;
use super::docx_fixture::{fixture, part, project};

fn docx(ops: &[EditOp]) -> Result<Vec<u8>, CodecError> {
    let bytes = fixture();
    apply_rich_ops(DocumentFormat::Docx, &bytes, &project(&bytes), ops)
}

#[test]
fn set_section_turns_the_page_to_landscape_with_a_header() {
    let op = EditOp::SetSection(SetSection {
        section: json!({
            "size": {"width": 842, "height": 595},
            "orientation": "landscape",
            "margins": {"top": 36, "right": 36, "bottom": 36, "left": 36},
            "header": {"blocks": [{"id": "", "type": "paragraph", "text": "Confidential"}]}
        }),
    });
    let out = docx(&[op]).expect("apply");
    let xml = part(&out, "word/document.xml");
    assert!(xml.contains("w:orient=\"landscape\""), "{xml}");
    assert!(xml.contains("w:headerReference"), "{xml}");
}

#[test]
fn set_section_refuses_raw_markup_in_a_running_part() {
    let op = EditOp::SetSection(SetSection {
        section: json!({"footer": {"blocks": [{"id": "", "type": "raw", "source": "<w:p/>"}]}}),
    });
    let error = docx(&[op]).expect_err("refused");
    assert!(matches!(error, CodecError::Refused { .. }), "{error:?}");
}

#[test]
fn set_style_adds_then_replaces_a_style_by_id() {
    let style = |font: &str| {
        EditOp::SetStyle(SetStyle {
            style: json!({"id": "Callout", "name": "Callout", "font": font}),
        })
    };
    let out = docx(&[style("Arial"), style("Georgia")]).expect("apply");
    let styles = part(&out, "word/styles.xml");
    assert!(styles.contains("w:styleId=\"Callout\""));
    assert!(styles.contains("Georgia"));
    assert!(!styles.contains("Arial\""));
}

#[test]
fn set_style_without_an_id_is_refused() {
    let op = EditOp::SetStyle(SetStyle {
        style: json!({"id": "", "name": "Nameless"}),
    });
    let error = docx(&[op]).expect_err("refused");
    assert!(matches!(error, CodecError::Refused { .. }), "{error:?}");
}

#[test]
fn apply_theme_on_a_package_without_a_theme_part_is_refused() {
    let op = EditOp::ApplyTheme(ApplyTheme {
        theme: json!({"heading_font": "Montserrat", "body_font": "Lora", "palette": ["112233"]}),
    });
    let error = docx(&[op]).expect_err("no theme part");
    assert!(matches!(error, CodecError::Refused { .. }), "{error:?}");
}

#[test]
fn apply_theme_refuses_a_malformed_theme() {
    let op = EditOp::ApplyTheme(ApplyTheme {
        theme: json!({"palette": "not-a-list"}),
    });
    let error = docx(&[op]).expect_err("refused");
    assert!(
        matches!(
            error,
            CodecError::Payload {
                what: "apply_theme.theme",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn document_level_ops_are_unsupported_on_markdown() {
    let ids = MarkdownCodec.project(SAMPLE_MD).expect("project");
    let op = EditOp::ApplyTheme(ApplyTheme {
        theme: json!({"body_font": "Lora"}),
    });
    let error =
        apply_rich_ops(DocumentFormat::Markdown, SAMPLE_MD, &ids, &[op]).expect_err("markdown");
    assert!(
        matches!(error, CodecError::UnsupportedOp { .. }),
        "{error:?}"
    );
}
