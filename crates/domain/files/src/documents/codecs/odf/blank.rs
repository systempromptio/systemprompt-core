//! Minimal valid `OpenDocument` packages new documents start from: the
//! `mimetype` entry first and stored, then the manifest, content, styles and
//! meta parts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{Cursor, Write};

use systemprompt_provider_contracts::documents::{CodecError, DocumentFormat};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::documents::codecs::zip_package::parse_error;

const NAMESPACES: &str = concat!(
    r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" "#,
    r#"xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" "#,
    r#"xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" "#,
    r#"xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" "#,
    r#"xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" "#,
    r#"xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" "#,
    r#"xmlns:xlink="http://www.w3.org/1999/xlink" "#,
    r#"xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" "#,
    r#"xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0" "#,
    r#"xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0" "#,
    r#"xmlns:of="urn:oasis:names:tc:opendocument:xmlns:of:1.2""#,
);

const DECLARATION: &str = r#"<?xml version="1.0" encoding="UTF-8"?>"#;

fn manifest(mime: &str) -> String {
    format!(
        "{DECLARATION}<manifest:manifest \
         xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" \
         manifest:version=\"1.3\"><manifest:file-entry manifest:full-path=\"/\" \
         manifest:version=\"1.3\" manifest:media-type=\"{mime}\"/><manifest:file-entry \
         manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/><manifest:file-entry \
         manifest:full-path=\"styles.xml\" manifest:media-type=\"text/xml\"/><manifest:file-entry \
         manifest:full-path=\"meta.xml\" manifest:media-type=\"text/xml\"/></manifest:manifest>"
    )
}

fn content(body: &str) -> String {
    format!(
        "{DECLARATION}<office:document-content {NAMESPACES} office:version=\"1.3\">\
         <office:body>{body}</office:body></office:document-content>"
    )
}

fn styles(extra: &str) -> String {
    format!(
        "{DECLARATION}<office:document-styles {NAMESPACES} office:version=\"1.3\">\
         <office:styles/>{extra}</office:document-styles>"
    )
}

fn meta() -> String {
    format!(
        "{DECLARATION}<office:document-meta {NAMESPACES} office:version=\"1.3\"><office:meta>\
         <meta:generator>systemprompt.io</meta:generator></office:meta></office:document-meta>"
    )
}

fn package(format: DocumentFormat, body: &str, extra_styles: &str) -> Result<Vec<u8>, CodecError> {
    let zip_error = |source| parse_error("zip container", source);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.start_file("mimetype", stored).map_err(zip_error)?;
    writer.write_all(format.mime_type().as_bytes())?;
    let parts = [
        ("META-INF/manifest.xml", manifest(format.mime_type())),
        ("content.xml", content(body)),
        ("styles.xml", styles(extra_styles)),
        ("meta.xml", meta()),
    ];
    for (name, xml) in parts {
        writer.start_file(name, deflated).map_err(zip_error)?;
        writer.write_all(xml.as_bytes())?;
    }
    Ok(writer.finish().map_err(zip_error)?.into_inner())
}

pub fn blank_odt() -> Result<Vec<u8>, CodecError> {
    package(
        DocumentFormat::Odt,
        "<office:text><text:p/></office:text>",
        "",
    )
}

pub fn blank_ods() -> Result<Vec<u8>, CodecError> {
    package(
        DocumentFormat::Ods,
        "<office:spreadsheet><table:table table:name=\"Sheet1\"><table:table-column/>\
         <table:table-row><table:table-cell/></table:table-row></table:table>\
         </office:spreadsheet>",
        "",
    )
}

pub fn blank_odp() -> Result<Vec<u8>, CodecError> {
    package(
        DocumentFormat::Odp,
        "<office:presentation><draw:page draw:name=\"page1\" \
         draw:master-page-name=\"Default\"><draw:frame presentation:class=\"title\" \
         svg:x=\"2cm\" svg:y=\"1cm\" svg:width=\"24cm\" svg:height=\"3cm\"><draw:text-box>\
         <text:p/></draw:text-box></draw:frame></draw:page></office:presentation>",
        "<office:automatic-styles><style:page-layout style:name=\"PM1\">\
         <style:page-layout-properties fo:page-width=\"28cm\" fo:page-height=\"15.75cm\" \
         style:print-orientation=\"landscape\"/></style:page-layout></office:automatic-styles>\
         <office:master-styles><style:master-page style:name=\"Default\" \
         style:page-layout-name=\"PM1\"/></office:master-styles>",
    )
}
