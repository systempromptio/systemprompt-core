//! External hyperlinks for the runs an edit writes.
//!
//! A `w:hyperlink` names a relationship of `word/document.xml`, so writing a
//! link adds an `External` hyperlink relationship to
//! `word/_rels/document.xml.rels`, or reuses one that already targets the
//! same URL. A package without that part, or a document whose root does not
//! declare the `r:` namespace, gets no hyperlinks: [`Links::id_for`] answers
//! `None` and the caller writes the link as underlined text instead.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::CodecError;

use super::parts::{malformed, read_part, replace_part};

const RELS: &str = "word/_rels/document.xml.rels";
const HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";

#[derive(Debug, Default)]
pub(super) struct Links {
    rels: Option<String>,
    taken: HashSet<String>,
    known: HashMap<String, String>,
    added: String,
}

impl Links {
    pub(super) fn load(bytes: &[u8], document: &str) -> Result<Self, CodecError> {
        let mut links = Self::default();
        if !root_tag(document).contains("xmlns:r=") {
            return Ok(links);
        }
        let Some(rels) = read_part(bytes, RELS)?.and_then(|raw| String::from_utf8(raw).ok()) else {
            return Ok(links);
        };
        for element in rels.split("<Relationship").skip(1) {
            let element = element.split('>').next().unwrap_or_default();
            let Some(id) = attr(element, "Id") else {
                continue;
            };
            links.taken.insert(id.to_owned());
            if attr(element, "Type") == Some(HYPERLINK)
                && attr(element, "TargetMode") == Some("External")
                && let Some(target) = attr(element, "Target")
            {
                links.known.insert(target.to_owned(), id.to_owned());
            }
        }
        links.rels = Some(rels);
        Ok(links)
    }

    pub(super) fn id_for(&mut self, href: &str) -> Option<String> {
        self.rels.as_ref()?;
        let target = escape(href).into_owned();
        if let Some(id) = self.known.get(&target) {
            return Some(id.clone());
        }
        let id = (1..)
            .map(|n: u32| format!("rIdLink{n}"))
            .find(|id| !self.taken.contains(id))?;
        self.taken.insert(id.clone());
        self.known.insert(target.clone(), id.clone());
        self.added.push_str(&format!(
            "<Relationship Id=\"{id}\" Type=\"{HYPERLINK}\" Target=\"{target}\" \
             TargetMode=\"External\"/>"
        ));
        Some(id)
    }

    pub(super) fn finish(self, bytes: Vec<u8>) -> Result<Vec<u8>, CodecError> {
        let Some(rels) = self.rels.filter(|_| !self.added.is_empty()) else {
            return Ok(bytes);
        };
        let at = rels
            .rfind("</Relationships>")
            .ok_or(malformed("the document relationships part is not closed"))?;
        let xml = format!("{}{}{}", &rels[..at], self.added, &rels[at..]);
        replace_part(&bytes, RELS, xml.as_bytes())
    }
}

pub(super) fn hyperlink(id: &str, runs: &str) -> String {
    format!("<w:hyperlink r:id=\"{id}\">{runs}</w:hyperlink>")
}

fn root_tag(document: &str) -> &str {
    document.find("<w:document").map_or("", |at| {
        document[at..].split('>').next().unwrap_or_default()
    })
}

fn attr<'a>(element: &'a str, name: &str) -> Option<&'a str> {
    let open = format!(" {name}=\"");
    let start = element.find(&open)? + open.len();
    let len = element[start..].find('"')?;
    Some(&element[start..start + len])
}
