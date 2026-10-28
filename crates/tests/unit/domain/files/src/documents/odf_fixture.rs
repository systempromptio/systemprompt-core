use std::io::{Cursor, Write};

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{DocumentCodec, EditOp, Projection};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::docx_fixture::entries;

pub const NAMESPACES: &str = concat!(
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
    r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
    r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:xlink="http://www.w3.org/1999/xlink" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
    r#"xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0""#,
);

pub fn content_xml(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- fixture -->\
         <office:document-content {NAMESPACES} office:version=\"1.3\"><office:body>{body}\
         </office:body></office:document-content>"
    )
}

pub fn package(mime: &str, body: &str) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.start_file("mimetype", stored).expect("mimetype");
    writer.write_all(mime.as_bytes()).expect("write");
    let styles = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><office:document-styles {NAMESPACES}>\
         <office:master-styles><style:master-page style:name=\"Default\"/>\
         </office:master-styles></office:document-styles>"
    );
    let parts = [
        ("META-INF/manifest.xml", "<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\"/>".to_owned()),
        ("content.xml", content_xml(body)),
        ("styles.xml", styles),
        ("Pictures/x.png", "png".to_owned()),
    ];
    for (name, xml) in parts {
        writer.start_file(name, deflated).expect("start");
        writer.write_all(xml.as_bytes()).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

pub fn id(value: &str) -> DocumentNodeId {
    DocumentNodeId::new(value)
}

pub fn hash_of(projection: &Projection, node: &str) -> String {
    projection
        .find(&id(node))
        .map(|n| n.hash8.clone())
        .expect("node exists")
}

pub fn text_of(projection: &Projection, node: &str) -> Option<String> {
    projection.find(&id(node)).and_then(|n| n.text.clone())
}

pub fn apply(codec: &dyn DocumentCodec, bytes: &[u8], ops: &[EditOp]) -> Vec<u8> {
    let base = codec.project(bytes).expect("project");
    codec.apply(bytes, &base, ops).expect("apply")
}

pub fn assert_only_content_changed(before: &[u8], after: &[u8]) {
    let old = entries(before);
    let new = entries(after);
    assert_eq!(
        old.iter().map(|e| e.0.clone()).collect::<Vec<_>>(),
        new.iter().map(|e| e.0.clone()).collect::<Vec<_>>()
    );
    for (old, new) in old.iter().zip(&new) {
        if old.0 == "content.xml" {
            assert_ne!(old.1, new.1, "content.xml changed");
        } else {
            assert_eq!(old, new, "{} is untouched", old.0);
        }
    }
    assert_mimetype_first_stored(after);
}

pub fn assert_mimetype_first_stored(bytes: &[u8]) {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("zip");
    let first = archive.by_index(0).expect("first entry");
    assert_eq!(first.name().expect("name"), "mimetype");
    assert_eq!(first.compression(), CompressionMethod::Stored);
}
