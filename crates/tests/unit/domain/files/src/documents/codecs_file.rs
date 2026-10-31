use systemprompt_files::documents::codecs::FileCodec;
use systemprompt_files::documents::codecs::file::{PREVIEW_IMAGE_BYTES, image_mime, is_pdf};
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentCodec, DocumentFormat, EditOp, TemplateParams, content_hash,
};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR";

#[test]
fn image_signatures_name_their_mime_and_anything_else_names_none() {
    assert_eq!(image_mime(PNG), Some("image/png"));
    assert_eq!(image_mime(b"\xff\xd8\xff\xe0rest"), Some("image/jpeg"));
    assert_eq!(image_mime(b"GIF87a...."), Some("image/gif"));
    assert_eq!(image_mime(b"GIF89a...."), Some("image/gif"));
    assert_eq!(image_mime(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
    assert_eq!(image_mime(b"RIFF\0\0\0\0WAVE"), None);
    assert_eq!(image_mime(b"RIFFWEBP"), None);
    assert_eq!(image_mime(b"%PDF-1.7"), None);
    assert!(is_pdf(b"%PDF-1.4\n"));
    assert!(!is_pdf(b"PDF-1.4"));
}

#[test]
fn a_file_projects_to_no_nodes_and_refuses_every_edit() {
    let codec = FileCodec;
    assert_eq!(codec.formats(), &[DocumentFormat::File]);
    assert!(codec.insert_kinds(DocumentFormat::File).is_empty());
    let bytes = b"opaque bytes".to_vec();
    let projection = codec.project(&bytes).expect("project");
    assert!(projection.root.is_empty());
    assert_eq!(projection.content_sha256, content_hash(&bytes));
    assert_eq!(codec.apply(&bytes, &projection, &[]).expect("noop"), bytes);
    let op = EditOp::AppendMarkdown {
        markdown: "more".to_owned(),
    };
    let error = codec
        .apply(&bytes, &projection, &[op])
        .expect_err("refused");
    assert!(matches!(
        error,
        CodecError::UnsupportedOp {
            op: "append_markdown",
            format: DocumentFormat::File
        }
    ));
    assert_eq!(codec.plain_text(&bytes).expect("text"), "");
    assert_eq!(
        codec
            .instantiate(&bytes, &TemplateParams::default())
            .expect("instantiate"),
        bytes
    );
}

#[test]
fn previews_inline_small_images_and_describe_everything_else() {
    let codec = FileCodec;
    let image = codec.preview_html(PNG).expect("png");
    assert!(image.starts_with("<img "));
    assert!(image.contains("src=\"data:image/png;base64,iVBORw0KGgo"));
    let pdf = codec.preview_html(b"%PDF-1.7 body").expect("pdf");
    assert!(pdf.contains("PDF document"));
    let other = codec.preview_html(b"zip?").expect("other");
    assert!(other.contains("no preview"));
    let mut huge = PNG.to_vec();
    huge.resize(PREVIEW_IMAGE_BYTES + 1, 0);
    assert!(
        codec
            .preview_html(&huge)
            .expect("huge")
            .contains("no preview")
    );
}
