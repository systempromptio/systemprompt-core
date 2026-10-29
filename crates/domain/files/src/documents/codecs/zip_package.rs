//! Zip package access shared by the office codecs: a view that serves edited
//! parts from memory and, on finish, copies every untouched entry's
//! compressed bytes, metadata and position verbatim. New parts are appended
//! after the original entries, so a leading `mimetype` entry stays first and
//! keeps its compression method.
//!
//! Every inflated read is bounded: a part may not inflate past
//! `PackageCaps::part_bytes`, the parts one package hands out may not
//! inflate past `PackageCaps::inflated_bytes` together, and an archive may
//! not list more than `PackageCaps::parts` entries. Codecs read under
//! `PackageCaps::CEILING`; the document service runs `inspect_package` under
//! the operator's configured caps before any codec sees uploaded bytes.
//!
//! Whether bytes are a package is decided by opening them, never by their
//! leading bytes: an archive reader finds the central directory from the end
//! of the input, so a package behind prepended bytes still opens. Bytes that
//! do not open are refused as a malformed package only when they start with
//! a local file header or hold an end-of-central-directory record where a
//! reader searches for one (the last 65,557 bytes); anything else is not a
//! package and passes through, so a signature that occurs by chance inside a
//! large binary does not refuse it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use systemprompt_provider_contracts::documents::CodecError;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::config::{
    PACKAGE_INFLATED_BYTES_CEILING, PACKAGE_PART_BYTES_CEILING, PACKAGE_PARTS_CEILING,
};

const CONTAINER: &str = "zip container";
const LOCAL_HEADER: &[u8] = b"PK\x03\x04";
const END_OF_CENTRAL_DIRECTORY: &[u8] = b"PK\x05\x06";
const END_RECORD_SEARCH: usize = 22 + u16::MAX as usize;

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

/// Inflation caps for one Office zip package: bytes of one part, bytes
/// across all parts, and listed entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageCaps {
    pub part_bytes: u64,
    pub inflated_bytes: u64,
    pub parts: u64,
}

impl PackageCaps {
    pub const CEILING: Self = Self {
        part_bytes: PACKAGE_PART_BYTES_CEILING,
        inflated_bytes: PACKAGE_INFLATED_BYTES_CEILING,
        parts: PACKAGE_PARTS_CEILING,
    };
}

const fn too_large(what: &'static str, size: u64, limit: u64) -> CodecError {
    CodecError::TooLarge { what, size, limit }
}

fn check_part_count(count: usize, caps: PackageCaps) -> Result<(), CodecError> {
    let count = u64::try_from(count).unwrap_or(u64::MAX);
    if count > caps.parts {
        return Err(too_large("zip package parts", count, caps.parts));
    }
    Ok(())
}

pub(crate) fn read_bounded(file: impl Read, limit: u64) -> Result<Vec<u8>, CodecError> {
    let mut raw = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut raw)?;
    let size = u64::try_from(raw.len()).unwrap_or(u64::MAX);
    if size > limit {
        return Err(too_large("zip package part (inflated bytes)", size, limit));
    }
    Ok(raw)
}

pub(crate) fn open_archive(bytes: &[u8]) -> Result<ZipArchive<Cursor<&[u8]>>, CodecError> {
    let archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_error)?;
    check_part_count(archive.len(), PackageCaps::CEILING)?;
    Ok(archive)
}

fn add_inflated(total: u64, part: &[u8], caps: PackageCaps) -> Result<u64, CodecError> {
    let total = total.saturating_add(u64::try_from(part.len()).unwrap_or(u64::MAX));
    if total > caps.inflated_bytes {
        return Err(too_large(
            "zip package (inflated bytes)",
            total,
            caps.inflated_bytes,
        ));
    }
    Ok(total)
}

fn carries_zip_signature(bytes: &[u8]) -> bool {
    if bytes.starts_with(LOCAL_HEADER) {
        return true;
    }
    let tail = &bytes[bytes.len().saturating_sub(END_RECORD_SEARCH)..];
    tail.windows(END_OF_CENTRAL_DIRECTORY.len())
        .any(|window| window == END_OF_CENTRAL_DIRECTORY)
}

pub fn inspect_package(bytes: &[u8], caps: PackageCaps) -> Result<(), CodecError> {
    let mut archive = match ZipArchive::new(Cursor::new(bytes)) {
        Ok(archive) => archive,
        Err(_) if !carries_zip_signature(bytes) => return Ok(()),
        Err(source) => return Err(zip_error(source)),
    };
    check_part_count(archive.len(), caps)?;
    let mut total = 0;
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(zip_error)?;
        let part = read_bounded(file, caps.part_bytes)?;
        total = add_inflated(total, &part, caps)?;
    }
    Ok(())
}

pub(crate) struct Package<'a> {
    archive: ZipArchive<Cursor<&'a [u8]>>,
    overrides: BTreeMap<String, String>,
    added: Vec<(String, String)>,
    inflated: u64,
}

impl<'a> Package<'a> {
    pub(crate) fn open(bytes: &'a [u8]) -> Result<Self, CodecError> {
        Ok(Self {
            archive: open_archive(bytes)?,
            overrides: BTreeMap::new(),
            added: Vec::new(),
            inflated: 0,
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
        let file = match self.archive.by_name(name) {
            Ok(file) => file,
            Err(ZipError::FileNotFound) => return Ok(None),
            Err(source) => return Err(zip_error(source)),
        };
        let caps = PackageCaps::CEILING;
        let raw = read_bounded(file, caps.part_bytes)?;
        self.inflated = add_inflated(self.inflated, &raw, caps)?;
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
