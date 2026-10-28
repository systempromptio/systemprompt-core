//! Whole-package edits for a `.docx` materialise: replaced and added parts
//! (binary, so media fits), the content-types entries a new part needs, and
//! one rewrite that copies every other entry's compressed bytes unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::io::{Cursor, Write};

use systemprompt_provider_contracts::documents::CodecError;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::parts::{malformed, read_part, zip_error};

const CONTENT_TYPES: &str = "[Content_Types].xml";

#[derive(Debug, Default)]
pub(super) struct PartEdits {
    parts: BTreeMap<String, Vec<u8>>,
    defaults: Vec<(String, String)>,
    overrides: Vec<(String, String)>,
}

impl PartEdits {
    pub(super) fn put(&mut self, name: &str, bytes: Vec<u8>) {
        self.parts.insert(name.to_owned(), bytes);
    }

    pub(super) fn get(&self, name: &str) -> Option<&[u8]> {
        self.parts.get(name).map(Vec::as_slice)
    }

    /// Registers a new part's content type by its extension.
    pub(super) fn default_type(&mut self, extension: &str, mime: &str) {
        if !self.defaults.iter().any(|(ext, _)| ext == extension) {
            self.defaults.push((extension.to_owned(), mime.to_owned()));
        }
    }

    /// Registers a new part's content type by its name.
    pub(super) fn override_type(&mut self, part: &str, content_type: &str) {
        self.overrides
            .push((format!("/{part}"), content_type.to_owned()));
    }

    pub(super) fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    pub(super) fn write(mut self, base: &[u8]) -> Result<Vec<u8>, CodecError> {
        if self.is_empty() {
            return Ok(base.to_vec());
        }
        if !self.defaults.is_empty() || !self.overrides.is_empty() {
            let types = content_types(base, &self.defaults, &self.overrides)?;
            self.parts.insert(CONTENT_TYPES.to_owned(), types.into_bytes());
        }
        let mut archive = ZipArchive::new(Cursor::new(base)).map_err(zip_error)?;
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for index in 0..archive.len() {
            let file = archive.by_index_raw(index).map_err(zip_error)?;
            let name = file.name().map_err(zip_error)?.into_owned();
            match self.parts.remove(&name) {
                Some(bytes) => {
                    drop(file);
                    writer.start_file(name, options).map_err(zip_error)?;
                    writer.write_all(&bytes)?;
                },
                None => writer.raw_copy_file(file).map_err(zip_error)?,
            }
        }
        for (name, bytes) in self.parts {
            writer.start_file(name, options).map_err(zip_error)?;
            writer.write_all(&bytes)?;
        }
        Ok(writer.finish().map_err(zip_error)?.into_inner())
    }
}

fn content_types(
    base: &[u8],
    defaults: &[(String, String)],
    overrides: &[(String, String)],
) -> Result<String, CodecError> {
    let raw = read_part(base, CONTENT_TYPES)?
        .ok_or(malformed("the package has no [Content_Types].xml part"))?;
    let xml =
        String::from_utf8(raw).map_err(|_utf8| malformed("[Content_Types].xml is not UTF-8"))?;
    let at = xml
        .rfind("</Types>")
        .ok_or(malformed("[Content_Types].xml is not closed"))?;
    let mut added = String::new();
    for (extension, mime) in defaults {
        if !xml.contains(&format!("Extension=\"{extension}\"")) {
            added.push_str(&format!(
                "<Default Extension=\"{extension}\" ContentType=\"{mime}\"/>"
            ));
        }
    }
    for (part, content_type) in overrides {
        if !xml.contains(&format!("PartName=\"{part}\"")) {
            added.push_str(&format!(
                "<Override PartName=\"{part}\" ContentType=\"{content_type}\"/>"
            ));
        }
    }
    Ok(format!("{}{added}{}", &xml[..at], &xml[at..]))
}
