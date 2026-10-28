use std::io::{Cursor, Read, Write};

use systemprompt_files::documents::codecs::docx::{DocxCodec, blank_docx};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    ContentAs, DocumentCodec, EditOp, NodeTarget, Projection,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub const IMAGE_BYTES: &[u8] = b"\x89PNG\r\n\x1a\nnot-really-an-image";

pub const DOCUMENT_XML: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    "\r\n<!-- fixture comment -->",
    r#"<?fixture-pi keep?>"#,
    r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" mc:Ignorable="w14">"#,
    "<w:body>",
    r#"<w:p w14:paraId="1A2B3C4D" w14:textId="77777777"><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Quarterly Report</w:t></w:r></w:p>"#,
    r#"<w:p w14:paraId="2B3C4D5E"><w:bookmarkStart w:id="0" w:name="intro"/><w:r><w:rPr><w:b/></w:rPr><w:t>Revenue grew</w:t></w:r><w:r><w:t xml:space="preserve"> strongly.</w:t></w:r><w:bookmarkEnd w:id="0"/></w:p>"#,
    r#"<w:p w14:paraId="3C4D5E6F"><w:pPr><w:pStyle w:val="ListParagraph"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>First point</w:t></w:r></w:p>"#,
    r#"<w:p w14:paraId="4D5E6F70"><w:r><mc:AlternateContent><mc:Choice Requires="w14"><w:drawing><wp:inline><a:graphic><a:graphicData><a:blip r:embed="rId9"/></a:graphicData></a:graphic></wp:inline></w:drawing></mc:Choice><mc:Fallback><w:pict/></mc:Fallback></mc:AlternateContent></w:r><w:r><w:t>Caption {{client}}</w:t></w:r></w:p>"#,
    r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:p w14:paraId="5E6F7081"><w:r><w:t>Region</w:t></w:r></w:p></w:tc><w:tc><w:p w14:paraId="6F708192"><w:r><w:t>Total</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    r#"<w:p w14:paraId="708192A3"><w:r><w:t>Prepared for {{</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>client</w:t></w:r><w:r><w:t>}} by Ops &amp; Finance.</w:t></w:r></w:p>"#,
    r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>"#,
    "</w:body></w:document>",
);

pub fn entries(bytes: &[u8]) -> Vec<(String, Vec<u8>, u32, u64)> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("zip");
    (0..archive.len())
        .map(|index| {
            let mut file = archive.by_index(index).expect("entry");
            let mut content = Vec::new();
            file.read_to_end(&mut content).expect("read");
            (
                file.name().expect("name").into_owned(),
                content,
                file.crc32(),
                file.compressed_size(),
            )
        })
        .collect()
}

pub fn part(bytes: &[u8], name: &str) -> String {
    entries(bytes)
        .into_iter()
        .find(|(entry, ..)| entry == name)
        .map(|(_, content, ..)| String::from_utf8(content).expect("utf8"))
        .expect("part present")
}

pub fn package_with(document_xml: &str) -> Vec<u8> {
    let blank = entries(&blank_docx().expect("blank"));
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content, ..) in &blank {
        let content = if name == "word/document.xml" {
            document_xml.as_bytes()
        } else {
            content.as_slice()
        };
        writer.start_file(name.as_str(), deflated).expect("start");
        writer.write_all(content).expect("write");
        if name == "word/styles.xml" {
            writer
                .start_file("word/media/image1.png", stored)
                .expect("start");
            writer.write_all(IMAGE_BYTES).expect("write");
            writer
                .start_file("word/settings.xml", deflated)
                .expect("start");
            writer
                .write_all(br#"<w:settings xmlns:w="urn:x"><w:zoom w:percent="100"/></w:settings>"#)
                .expect("write");
        }
    }
    writer.finish().expect("finish").into_inner()
}

pub fn fixture() -> Vec<u8> {
    package_with(DOCUMENT_XML)
}

pub fn project(bytes: &[u8]) -> Projection {
    DocxCodec.project(bytes).expect("project")
}

pub fn node_id(id: &str) -> DocumentNodeId {
    DocumentNodeId::new(id)
}

pub fn hash_of(projection: &Projection, id: &str) -> String {
    projection.find(&node_id(id)).expect("node").hash8.clone()
}

pub fn text_of(projection: &Projection, id: &str) -> String {
    projection
        .find(&node_id(id))
        .and_then(|node| node.text.clone())
        .expect("node text")
}

pub fn apply(bytes: &[u8], ops: &[EditOp]) -> Vec<u8> {
    DocxCodec.apply(bytes, &project(bytes), ops).expect("apply")
}

pub fn replace(bytes: &[u8], id: &str, text: &str) -> Vec<u8> {
    let projection = project(bytes);
    apply(
        bytes,
        &[EditOp::ReplaceText {
            node: NodeTarget::Id(node_id(id)),
            expected_hash: Some(hash_of(&projection, id)),
            text: text.to_owned(),
            content_as: ContentAs::Text,
        }],
    )
}
