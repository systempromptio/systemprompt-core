//! Zip-container access: read one part, or rewrite one part while copying
//! every other entry's compressed bytes, metadata and order unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{Cursor, Read, Write};

use systemprompt_provider_contracts::documents::CodecError;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub(super) const DOCUMENT: &str = "word/document.xml";
pub(super) const STYLES: &str = "word/styles.xml";
const CONTAINER: &str = "zip container";

pub(super) const fn malformed(detail: &'static str) -> CodecError {
    CodecError::Malformed { detail }
}

pub(super) fn zip_error(source: ZipError) -> CodecError {
    CodecError::Parse {
        part: CONTAINER.to_owned(),
        source: Box::new(source),
    }
}

pub(super) fn read_part(bytes: &[u8], name: &str) -> Result<Option<Vec<u8>>, CodecError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_error)?;
    let mut file = match archive.by_name(name) {
        Ok(file) => file,
        Err(ZipError::FileNotFound) => return Ok(None),
        Err(source) => return Err(zip_error(source)),
    };
    let mut out = Vec::new();
    file.read_to_end(&mut out)?;
    Ok(Some(out))
}

pub(super) fn read_document(bytes: &[u8]) -> Result<String, CodecError> {
    let raw = read_part(bytes, DOCUMENT)?.ok_or(CodecError::Malformed {
        detail: "the package has no word/document.xml part",
    })?;
    String::from_utf8(raw).map_err(|source| CodecError::Parse {
        part: DOCUMENT.to_owned(),
        source: Box::new(source),
    })
}

pub(super) fn replace_part(
    bytes: &[u8],
    name: &str,
    content: &[u8],
) -> Result<Vec<u8>, CodecError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_error)?;
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let file = archive.by_index_raw(index).map_err(zip_error)?;
        if file.name().map_err(zip_error)? == name {
            let method = match file.compression() {
                CompressionMethod::Stored => CompressionMethod::Stored,
                _ => CompressionMethod::Deflated,
            };
            let mut options = SimpleFileOptions::default().compression_method(method);
            if let Some(modified) = file.last_modified() {
                options = options.last_modified_time(modified);
            }
            drop(file);
            writer.start_file(name, options).map_err(zip_error)?;
            writer.write_all(content)?;
        } else {
            writer.raw_copy_file(file).map_err(zip_error)?;
        }
    }
    Ok(writer.finish().map_err(zip_error)?.into_inner())
}

pub(super) fn add_part(bytes: &[u8], name: &str, content: &[u8]) -> Result<Vec<u8>, CodecError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_error)?;
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let file = archive.by_index_raw(index).map_err(zip_error)?;
        writer.raw_copy_file(file).map_err(zip_error)?;
    }
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.start_file(name, options).map_err(zip_error)?;
    writer.write_all(content)?;
    Ok(writer.finish().map_err(zip_error)?.into_inner())
}
