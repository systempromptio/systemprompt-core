use std::io::Write;

use systemprompt_files::documents::model::BlockKind;
use systemprompt_files::documents::model::formats::render_rich_text;
use systemprompt_files::documents::service::ImportRequest;
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentFormat;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, harness, person, trace, unique,
};

const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0,
    0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 56, 97, 163, 1, 0, 2, 252,
    1, 45, 255, 131, 58, 87, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

const DOCUMENT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body>
<w:p><w:r><w:t>Before the picture</w:t></w:r></w:p>
<w:p><w:r><w:drawing><wp:inline><wp:extent cx="914400" cy="914400"/><wp:docPr id="1" name="p"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="0" name="p"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rId9"/></pic:blipFill><pic:spPr/></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
</w:body></w:document>"#;

const ODT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" office:version="1.3"><office:body><office:text>
<text:p>Before the picture</text:p>
<text:p>Beside it<draw:frame svg:width="2.54cm" svg:height="2.54cm"><draw:image xlink:href="Pictures/p.png"/></draw:frame></text:p>
<text:list><text:list-item><text:p>A kept list item</text:p></text:list-item></text:list>
<table:table><table:table-row><table:table-cell><text:p>A kept table cell</text:p></table:table-cell></table:table-row></table:table>
</office:text></office:body></office:document-content>"#;

fn package(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        zip.start_file(*name, SimpleFileOptions::default())
            .expect("start part");
        zip.write_all(bytes).expect("write part");
    }
    zip.finish().expect("finish package").into_inner()
}

fn docx() -> Vec<u8> {
    package(&[
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        ("word/document.xml", DOCUMENT_XML.as_bytes()),
        (
            "word/_rels/document.xml.rels",
            br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/p.png"/></Relationships>"#,
        ),
        ("word/media/p.png", PNG),
    ])
}

fn odt() -> Vec<u8> {
    package(&[
        ("mimetype", b"application/vnd.oasis.opendocument.text"),
        ("content.xml", ODT_XML.as_bytes()),
        ("Pictures/p.png", PNG),
    ])
}

async fn imported(
    h: &Harness,
    owner: &AuthenticatedUser,
    name: &str,
    format: DocumentFormat,
    bytes: Vec<u8>,
) -> DocumentId {
    h.service
        .import(
            owner,
            ImportRequest {
                folder: None,
                name: name.to_owned(),
                format: Some(format),
                mime_type: None,
                bytes,
                author: api_author(),
            },
            &trace(),
        )
        .await
        .expect("import")
        .0
        .id
}

fn draws_a_picture(pdf: &[u8]) -> bool {
    pdf.windows(6).any(|window| window == b"/Image")
}

#[tokio::test]
async fn a_docx_picture_reaches_the_pdf_service_output() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("pdf-docx-picture");
    let id = imported(
        &h,
        &owner,
        &format!("{}.docx", unique("pic")),
        DocumentFormat::Docx,
        docx(),
    )
    .await;
    let rendered = h
        .service
        .render_pdf(&owner, &id, None, &trace())
        .await
        .expect("render");
    assert!(draws_a_picture(&rendered.bytes));
}

#[tokio::test]
async fn an_odt_picture_reaches_the_pdf_service_output() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("pdf-odt-picture");
    let id = imported(
        &h,
        &owner,
        &format!("{}.odt", unique("pic")),
        DocumentFormat::Odt,
        odt(),
    )
    .await;
    let rendered = h
        .service
        .render_pdf(&owner, &id, None, &trace())
        .await
        .expect("render");
    assert!(draws_a_picture(&rendered.bytes));
}

#[test]
fn an_odt_renders_the_lists_tables_and_pictures_it_keeps_as_is() {
    let model = render_rich_text(DocumentFormat::Odt, &odt()).expect("model");
    let kinds = &model.blocks;
    assert!(kinds.iter().any(|b| b.text == "Beside it"));
    assert!(
        kinds
            .iter()
            .any(|b| matches!(&b.kind, BlockKind::Image { blob, width, .. }
        if blob == "Pictures/p.png" && *width == Some(96)))
    );
    assert!(
        kinds
            .iter()
            .any(|b| matches!(b.kind, BlockKind::ListItem { .. }) && b.text == "A kept list item")
    );
    assert!(
        kinds
            .iter()
            .any(|b| matches!(b.kind, BlockKind::Table { .. }))
    );
    assert!(
        !kinds
            .iter()
            .any(|b| matches!(b.kind, BlockKind::Raw { .. }))
    );
}
