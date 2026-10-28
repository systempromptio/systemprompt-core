use std::str::FromStr;

use systemprompt_models::documents::DocumentFormat;

#[test]
fn format_wire_names_round_trip() {
    for format in DocumentFormat::ALL {
        assert_eq!(DocumentFormat::from_str(format.as_str()), Ok(format));
        let json = serde_json::to_value(format).expect("serialize");
        assert_eq!(json, serde_json::json!(format.as_str()));
    }
    assert!(DocumentFormat::from_str("pdf").is_err());
}

#[test]
fn format_extension_round_trip() {
    for format in DocumentFormat::ALL {
        assert_eq!(
            DocumentFormat::from_extension(format.extension()),
            Some(format)
        );
    }
    assert_eq!(DocumentFormat::Markdown.extension(), "md");
    assert_eq!(DocumentFormat::Text.extension(), "txt");
    assert_eq!(
        DocumentFormat::from_extension(".DOCX"),
        Some(DocumentFormat::Docx)
    );
    assert_eq!(
        DocumentFormat::from_extension("htm"),
        Some(DocumentFormat::Html)
    );
    assert_eq!(DocumentFormat::from_extension("pdf"), None);
    assert_eq!(
        DocumentFormat::from_extension("csv"),
        Some(DocumentFormat::Text)
    );
}

#[test]
fn format_mime_types() {
    assert_eq!(
        DocumentFormat::Xlsx.mime_type(),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );
    assert_eq!(DocumentFormat::Markdown.mime_type(), "text/markdown");
    assert_eq!(
        DocumentFormat::Odp.mime_type(),
        "application/vnd.oasis.opendocument.presentation"
    );
}

#[test]
fn office_and_merge_partition_formats() {
    for format in DocumentFormat::ALL.into_iter().filter(|f| !f.is_file()) {
        assert_ne!(format.is_office(), format.supports_three_way_merge());
    }
    assert!(!DocumentFormat::File.is_office());
    assert!(!DocumentFormat::File.supports_three_way_merge());
    assert!(DocumentFormat::Html.supports_three_way_merge());
    assert!(DocumentFormat::Pptx.is_office());
}
