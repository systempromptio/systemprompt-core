//! `word/numbering.xml` for the list items a materialise writes.
//!
//! Every numbering definition the model's list items use gets its own
//! abstract numbering (its id stamped as `w:name`) and one `w:num`; a list
//! with no definition uses the built-in bullet or decimal list, added the
//! first time it is needed. A definition the base already holds with the
//! same levels keeps its `numId`, so untouched lists stay as they were.
//! New abstract numberings go before the first `w:num` and new `w:num`s at
//! the end, the order the schema requires; a package without the part gets
//! one, with its content type and relationship.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_provider_contracts::documents::CodecError;

use super::import::part_text;
use super::links::Links;
use super::package::PartEdits;
use super::parts::malformed;
use super::read_numbering::{BUILTIN_BULLET, BUILTIN_DECIMAL, NUMBERING, NumberingMap};
use super::write::escape_attr;
use crate::documents::model::document::{
    DocumentSettings, NumberFormat, NumberingDef, NumberingLevel,
};

const CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

#[derive(Debug, Default)]
pub(super) struct Numbering {
    base: NumberingMap,
    xml: Option<String>,
    assigned: HashMap<String, u32>,
    abstracts: String,
    nums: String,
    next_abstract: u32,
    next_num: u32,
}

impl Numbering {
    pub(super) fn load(base: &[u8]) -> Result<Self, CodecError> {
        let xml = part_text(base, NUMBERING)?;
        let map = xml.as_deref().map(NumberingMap::read).unwrap_or_default();
        Ok(Self {
            next_abstract: map.max_abstract + 1,
            next_num: map.max_num + 1,
            base: map,
            xml,
            ..Self::default()
        })
    }

    pub(super) fn num_id(
        &mut self,
        def: Option<&str>,
        ordered: bool,
        settings: &DocumentSettings,
    ) -> u32 {
        let builtin = if ordered {
            BUILTIN_DECIMAL
        } else {
            BUILTIN_BULLET
        };
        let def = def
            .and_then(|id| settings.numbering(id))
            .cloned()
            .unwrap_or_else(|| builtin_def(builtin, ordered));
        if let Some(num) = self.assigned.get(&def.id) {
            return *num;
        }
        let held = self.base.num_of.get(&def.id).and_then(|num| {
            let same = self.base.by_num.get(num).is_some_and(|base| {
                base.levels == def.levels || def.id == BUILTIN_BULLET || def.id == BUILTIN_DECIMAL
            });
            same.then(|| num.parse::<u32>().ok()).flatten()
        });
        let num = held.unwrap_or_else(|| self.add(&def));
        self.assigned.insert(def.id, num);
        num
    }

    fn add(&mut self, def: &NumberingDef) -> u32 {
        let (abs, num) = (self.next_abstract, self.next_num);
        self.next_abstract += 1;
        self.next_num += 1;
        self.abstracts.push_str(&format!(
            "<w:abstractNum w:abstractNumId=\"{abs}\"><w:multiLevelType \
             w:val=\"hybridMultilevel\"/><w:name w:val=\"{}\"/>",
            escape_attr(&def.id)
        ));
        for (ilvl, level) in def.levels.iter().enumerate().take(9) {
            self.abstracts.push_str(&level_xml(ilvl, level));
        }
        self.abstracts.push_str("</w:abstractNum>");
        self.nums.push_str(&format!(
            "<w:num w:numId=\"{num}\"><w:abstractNumId w:val=\"{abs}\"/></w:num>"
        ));
        num
    }

    pub(super) fn finish(self, edits: &mut PartEdits, links: &mut Links) -> Result<(), CodecError> {
        if self.nums.is_empty() {
            return Ok(());
        }
        let xml = if let Some(xml) = self.xml {
            let close = xml
                .rfind("</w:numbering>")
                .ok_or(malformed("word/numbering.xml is not closed"))?;
            let first_num = xml.find("<w:num ").or_else(|| xml.find("<w:num>"));
            let at = first_num.unwrap_or(close).min(close);
            format!(
                "{}{}{}{}{}",
                &xml[..at],
                self.abstracts,
                &xml[at..close],
                self.nums,
                &xml[close..]
            )
        } else {
            links
                .part_id("numbering", "numbering.xml")
                .ok_or(malformed("the document has no relationships part"))?;
            edits.override_type(NUMBERING, CONTENT_TYPE);
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:numbering \
                 xmlns:w=\"{W_NS}\">{}{}</w:numbering>",
                self.abstracts, self.nums
            )
        };
        edits.put(NUMBERING, xml.into_bytes());
        Ok(())
    }
}

fn level_xml(ilvl: usize, level: &NumberingLevel) -> String {
    let default_indent = (ilvl as u32).saturating_add(1).saturating_mul(720);
    let indent = level
        .indent
        .map_or(default_indent, |points| points.saturating_mul(20));
    let text = if level.text.is_empty() {
        default_text(level.format, ilvl)
    } else {
        level.text.clone()
    };
    format!(
        "<w:lvl w:ilvl=\"{ilvl}\"><w:start w:val=\"{}\"/><w:numFmt w:val=\"{}\"/><w:lvlText \
         w:val=\"{}\"/><w:lvlJc w:val=\"left\"/><w:pPr><w:ind w:left=\"{indent}\" \
         w:hanging=\"360\"/></w:pPr></w:lvl>",
        level.start,
        level.format.ooxml(),
        escape_attr(&text)
    )
}

fn default_text(format: NumberFormat, ilvl: usize) -> String {
    match format {
        NumberFormat::Bullet => ["\u{2022}", "\u{25E6}", "\u{25AA}"][ilvl % 3].to_owned(),
        _ => format!("%{}.", ilvl + 1),
    }
}

fn builtin_def(id: &str, ordered: bool) -> NumberingDef {
    let formats = if ordered {
        [
            NumberFormat::Decimal,
            NumberFormat::LowerLetter,
            NumberFormat::LowerRoman,
        ]
    } else {
        [NumberFormat::Bullet; 3]
    };
    NumberingDef {
        id: id.to_owned(),
        levels: (0..9)
            .map(|ilvl| NumberingLevel {
                format: formats[ilvl % 3],
                text: default_text(formats[ilvl % 3], ilvl),
                start: 1,
                indent: None,
            })
            .collect(),
    }
}
