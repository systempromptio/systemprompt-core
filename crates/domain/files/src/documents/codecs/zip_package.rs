//! Zip package access shared by the office codecs: a view that serves edited
//! parts from memory and, on finish, copies every untouched entry's
//! compressed bytes, metadata and position verbatim. New parts are appended
//! after the original entries, so a leading `mimetype` entry stays first and
//! keeps its compression method.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use systemprompt_provider_contracts::documents::CodecError;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const CONTAINER: &str = "zip container";

pub(crate) fn parse_error(
    part: &str,
    source: impl std::error::Error + Send + Sync + 'static,
) -> CodecError {
    CodecError::Parse {
        part: part.to_owned(),
        source: Box::new(source),
    }
}

fn zip_error(source: ZipError) -> CodecError {
    parse_error(CONTAINER, source)
}

pub(crate) struct Package<'a> {
    archive: ZipArchive<Cursor<&'a [u8]>>,
    overrides: BTreeMap<String, String>,
    added: Vec<(String, String)>,
}

impl<'a> Package<'a> {
    pub(crate) fn open(bytes: &'a [u8]) -> Result<Self, CodecError> {
        Ok(Self {
            archive: ZipArchive::new(Cursor::new(bytes)).map_err(zip_error)?,
            overrides: BTreeMap::new(),
            added: Vec::new(),
        })
    }

    pub(crate) fn names(&self) -> Result<Vec<String>, CodecError> {
        let mut names = self
            .archive
            .file_names()
            .map(|name| name.map(std::borrow::Cow::into_owned))
            .collect::<Result<Vec<_>, _>>()
            .map_err(zip_error)?;
        names.extend(self.added.iter().map(|(name, _)| name.clone()));
        Ok(names)
    }

    pub(crate) fn read(&mut self, name: &str) -> Result<Option<String>, CodecError> {
        if let Some(xml) = self.overrides.get(name) {
            return Ok(Some(xml.clone()));
        }
        if let Some((_, xml)) = self.added.iter().find(|(added, _)| added == name) {
            return Ok(Some(xml.clone()));
        }
        let mut file = match self.archive.by_name(name) {
            Ok(file) => file,
            Err(ZipError::FileNotFound) => return Ok(None),
            Err(source) => return Err(zip_error(source)),
        };
        let mut raw = Vec::new();
        file.read_to_end(&mut raw)?;
        String::from_utf8(raw)
            .map(Some)
            .map_err(|source| parse_error(name, source))
    }

    pub(crate) fn require(&mut self, name: &str) -> Result<String, CodecError> {
        self.read(name)?.ok_or(CodecError::Malformed {
            detail: "a part referenced by the presentation is missing from the package",
        })
    }

    pub(crate) fn write(&mut self, name: &str, xml: String) {
        if let Some(entry) = self.added.iter_mut().find(|(added, _)| added == name) {
            entry.1 = xml;
        } else if self.archive.index_for_name(name).is_some() {
            self.overrides.insert(name.to_owned(), xml);
        } else {
            self.added.push((name.to_owned(), xml));
        }
    }

    pub(crate) fn finish(mut self) -> Result<Vec<u8>, CodecError> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for index in 0..self.archive.len() {
            let file = self.archive.by_index_raw(index).map_err(zip_error)?;
            let name = file.name().map_err(zip_error)?.into_owned();
            let Some(xml) = self.overrides.get(&name) else {
                writer.raw_copy_file(file).map_err(zip_error)?;
                continue;
            };
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
            writer.write_all(xml.as_bytes())?;
        }
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, xml) in &self.added {
            writer
                .start_file(name.as_str(), options)
                .map_err(zip_error)?;
            writer.write_all(xml.as_bytes())?;
        }
        Ok(writer.finish().map_err(zip_error)?.into_inner())
    }
}
