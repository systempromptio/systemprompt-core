//! A minimal, valid `.docx` package written part by part.
//!
//! The parts are hand-written rather than generated with a document builder:
//! six small XML files need no extra dependency, and fixed entry timestamps
//! make the package bytes — and so its content hash — deterministic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{Cursor, Write};

use systemprompt_provider_contracts::documents::CodecError;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use super::parts::zip_error;

const DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn content_types() -> String {
    format!(
        r#"{DECL}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#
    )
}

fn package_rels() -> String {
    format!(
        r#"{DECL}<Relationships xmlns="{REL_NS}"><Relationship Id="rId1" Type="{OFFICE_REL}/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#
    )
}

fn document_rels() -> String {
    format!(
        r#"{DECL}<Relationships xmlns="{REL_NS}"><Relationship Id="rId1" Type="{OFFICE_REL}/styles" Target="styles.xml"/></Relationships>"#
    )
}

fn document() -> String {
    format!(
        r#"{DECL}<w:document xmlns:w="{W_NS}" xmlns:r="{OFFICE_REL}"><w:body><w:p/><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="708" w:footer="708" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    )
}

fn heading(id: &str, name: &str, outline: u8, size: u8) -> String {
    format!(
        r#"<w:style w:type="paragraph" w:styleId="{id}"><w:name w:val="{name}"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="120"/><w:outlineLvl w:val="{outline}"/></w:pPr><w:rPr><w:b/><w:sz w:val="{size}"/></w:rPr></w:style>"#
    )
}

fn styles() -> String {
    format!(
        r#"{DECL}<w:styles xmlns:w="{W_NS}"><w:docDefaults><w:rPrDefault><w:rPr><w:sz w:val="22"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="160"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:rPr><w:sz w:val="56"/></w:rPr></w:style>{}{}{}</w:styles>"#,
        heading("Heading1", "heading 1", 0, 32),
        heading("Heading2", "heading 2", 1, 28),
        heading("Heading3", "heading 3", 2, 24),
    )
}

fn core_properties() -> String {
    format!(
        r#"{DECL}<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:title></dc:title></cp:coreProperties>"#
    )
}

pub fn blank_docx() -> Result<Vec<u8>, CodecError> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default());
    // Why: Word expects [Content_Types].xml to be the first entry of the package.
    let entries = [
        ("[Content_Types].xml", content_types()),
        ("_rels/.rels", package_rels()),
        ("word/document.xml", document()),
        ("word/_rels/document.xml.rels", document_rels()),
        ("word/styles.xml", styles()),
        ("docProps/core.xml", core_properties()),
    ];
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in entries {
        writer.start_file(name, options).map_err(zip_error)?;
        writer.write_all(content.as_bytes())?;
    }
    Ok(writer.finish().map_err(zip_error)?.into_inner())
}
