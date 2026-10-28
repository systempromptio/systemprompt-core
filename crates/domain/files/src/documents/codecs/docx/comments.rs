//! Document comments written as native Word comments.
//!
//! Each comment anchored to a paragraph becomes a `w:comment` in
//! `word/comments.xml`, spanning that paragraph's runs between
//! `w:commentRangeStart` and `w:commentRangeEnd`, with a
//! `w:commentReference` run after the range. A package that already holds
//! comments keeps them and the new ones are numbered after its highest id;
//! one without a comments part gains the part, its content-type override and
//! its relationship. A comment whose node is not a paragraph of the body or a
//! table cell is left out, since Word anchors comments to runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::escape;
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::CodecError;

use super::markup::{self, Splice, run};
use super::parts::{self, add_part, malformed, read_part, replace_part};
use super::scan;

const COMMENTS: &str = "word/comments.xml";
const RELS: &str = "word/_rels/document.xml.rels";
const CONTENT_TYPES: &str = "[Content_Types].xml";
const COMMENTS_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";
const COMMENTS_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeComment {
    pub node: DocumentNodeId,
    pub author: String,
    pub date: String,
    pub text: String,
}

pub fn with_comments(bytes: &[u8], comments: &[NativeComment]) -> Result<Vec<u8>, CodecError> {
    let src = parts::read_document(bytes)?;
    let doc = scan::parse(&src)?;
    let existing = read_part(bytes, COMMENTS)?
        .map(|raw| utf8(COMMENTS, raw))
        .transpose()?;
    let mut next = existing.as_deref().map_or(0, |xml| highest_id(xml) + 1);
    let paragraphs = doc.paragraphs();
    let mut splices = Vec::new();
    let mut entries = String::new();
    for (order, comment) in comments.iter().enumerate() {
        let Some(para) = paragraphs
            .iter()
            .find(|para| para.id == comment.node.as_str())
        else {
            continue;
        };
        let xml = &src[para.range.clone()];
        if xml.ends_with("/>") || !xml.ends_with("</w:p>") {
            continue;
        }
        let start = para
            .ppr
            .as_ref()
            .map_or(para.range.start + para.open_tag.len(), |ppr| ppr.range.end);
        let end = para.range.end - "</w:p>".len();
        splices.push(Splice {
            range: start..start,
            text: format!("<w:commentRangeStart w:id=\"{next}\"/>"),
            order,
        });
        splices.push(Splice {
            range: end..end,
            text: format!(
                "<w:commentRangeEnd w:id=\"{next}\"/><w:r><w:commentReference w:id=\"{next}\"/></w:r>"
            ),
            order,
        });
        entries.push_str(&format!(
            "<w:comment w:id=\"{next}\" w:author=\"{}\" w:date=\"{}\"><w:p>{}</w:p></w:comment>",
            escape(comment.author.as_str()),
            escape(comment.date.as_str()),
            run(None, &comment.text),
        ));
        next += 1;
    }
    if entries.is_empty() {
        return Ok(bytes.to_vec());
    }
    let edited = markup::apply_splices(&src, splices)?;
    let bytes = replace_part(bytes, parts::DOCUMENT, edited.as_bytes())?;
    if let Some(xml) = existing {
        let at = xml
            .rfind("</w:comments>")
            .ok_or(malformed("the comments part is not closed"))?;
        let merged = format!("{}{entries}{}", &xml[..at], &xml[at..]);
        replace_part(&bytes, COMMENTS, merged.as_bytes())
    } else {
        let part = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <w:comments xmlns:w=\"{W_NS}\">{entries}</w:comments>"
        );
        let bytes = add_part(&bytes, COMMENTS, part.as_bytes())?;
        let bytes = declare_type(&bytes)?;
        relate(&bytes)
    }
}

fn highest_id(xml: &str) -> u32 {
    xml.match_indices("<w:comment ")
        .filter_map(|(at, _)| {
            let tag = xml[at..].split('>').next()?;
            let start = tag.find("w:id=\"")? + "w:id=\"".len();
            let len = tag[start..].find('"')?;
            tag[start..start + len].parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0)
}

fn utf8(name: &str, raw: Vec<u8>) -> Result<String, CodecError> {
    String::from_utf8(raw).map_err(|source| CodecError::Parse {
        part: name.to_owned(),
        source: Box::new(source),
    })
}

fn utf8_part(bytes: &[u8], name: &str) -> Result<String, CodecError> {
    let raw = read_part(bytes, name)?.ok_or(malformed("a required package part is missing"))?;
    utf8(name, raw)
}

fn declare_type(bytes: &[u8]) -> Result<Vec<u8>, CodecError> {
    let types = utf8_part(bytes, CONTENT_TYPES)?;
    let at = types
        .rfind("</Types>")
        .ok_or(malformed("the content types part is not closed"))?;
    let merged = format!(
        "{}<Override PartName=\"/{COMMENTS}\" ContentType=\"{COMMENTS_TYPE}\"/>{}",
        &types[..at],
        &types[at..]
    );
    replace_part(bytes, CONTENT_TYPES, merged.as_bytes())
}

fn relate(bytes: &[u8]) -> Result<Vec<u8>, CodecError> {
    let rels = utf8_part(bytes, RELS)?;
    let at = rels
        .rfind("</Relationships>")
        .ok_or(malformed("the document relationships part is not closed"))?;
    let id = (1..=u32::MAX)
        .map(|n| format!("rIdComments{n}"))
        .find(|id| !rels.contains(&format!("Id=\"{id}\"")))
        .ok_or(malformed("no relationship id is free"))?;
    let merged = format!(
        "{}<Relationship Id=\"{id}\" Type=\"{COMMENTS_REL}\" Target=\"comments.xml\"/>{}",
        &rels[..at],
        &rels[at..]
    );
    replace_part(bytes, RELS, merged.as_bytes())
}
