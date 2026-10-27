//! Relationship resolution for the `.pptx` codec.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::CodecError;

use crate::documents::codecs::xml_tree::parse_xml;
use crate::documents::codecs::zip_package::Package;

#[derive(Debug, Clone)]
pub(super) struct Relationship {
    pub id: String,
    pub rel_type: String,
    pub target: String,
    pub start: usize,
    pub end: usize,
}

pub(super) fn rels_part(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

fn resolve(base: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return absolute.to_owned();
    }
    let mut segments: Vec<&str> = base.split('/').collect();
    segments.pop();
    for segment in target.split('/') {
        match segment {
            ".." => {
                segments.pop();
            },
            "." | "" => {},
            other => segments.push(other),
        }
    }
    segments.join("/")
}

pub(super) fn relationships(
    package: &mut Package<'_>,
    part: &str,
) -> Result<(String, Vec<Relationship>), CodecError> {
    let name = rels_part(part);
    let Some(xml) = package.read(&name)? else {
        return Ok((String::new(), Vec::new()));
    };
    let root = parse_xml(&name, &xml)?;
    let rels = root
        .children_named("Relationship")
        .map(|rel| Relationship {
            id: rel.attr("Id").unwrap_or_default().to_owned(),
            rel_type: rel.attr("Type").unwrap_or_default().to_owned(),
            target: resolve(part, rel.attr("Target").unwrap_or_default()),
            start: rel.start,
            end: rel.end,
        })
        .collect();
    Ok((xml, rels))
}
