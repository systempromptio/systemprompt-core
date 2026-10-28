//! `word/numbering.xml` read as numbering definitions.
//!
//! Each `w:num` is one definition over its abstract numbering. A definition
//! is named by its abstract numbering's `w:name` (the writer stamps the
//! model's id there) the first time that name is seen, else `num<numId>`.
//! The writer's own built-in bullet and decimal lists ([`BUILTIN_BULLET`],
//! [`BUILTIN_DECIMAL`]) read as a list item with no numbering, so a list
//! the model wrote without a definition reads back without one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{HashMap, HashSet};

use crate::documents::codecs::xml_tree::{Element, parse_xml};
use crate::documents::model::document::{NumberFormat, NumberingDef, NumberingLevel};

use super::read::points;

pub(super) const NUMBERING: &str = "word/numbering.xml";
pub(super) const BUILTIN_BULLET: &str = "sp-bullet";
pub(super) const BUILTIN_DECIMAL: &str = "sp-decimal";

#[derive(Debug, Default)]
pub(super) struct NumberingMap {
    /// `numId` to the definition it stands for.
    pub(super) by_num: HashMap<String, NumberingDef>,
    /// Definition id to `numId`, for the writer.
    pub(super) num_of: HashMap<String, String>,
    /// The highest abstract and concrete numbering ids in the part.
    pub(super) max_abstract: u32,
    pub(super) max_num: u32,
}

impl NumberingMap {
    pub(super) fn read(xml: &str) -> Self {
        let mut map = Self::default();
        let Ok(root) = parse_xml(NUMBERING, xml) else {
            return map;
        };
        let mut abstracts: HashMap<&str, (Option<&str>, Vec<NumberingLevel>)> = HashMap::new();
        for abs in root.children_named("w:abstractNum") {
            let Some(id) = abs.attr("w:abstractNumId") else {
                continue;
            };
            map.max_abstract = map.max_abstract.max(id.parse().unwrap_or(0));
            let name = abs.child("w:name").and_then(|n| n.attr("w:val"));
            abstracts.insert(id, (name, abs.children_named("w:lvl").map(level).collect()));
        }
        let mut named = HashSet::new();
        for num in root.children_named("w:num") {
            let Some(num_id) = num.attr("w:numId") else {
                continue;
            };
            map.max_num = map.max_num.max(num_id.parse().unwrap_or(0));
            let abs = num.child("w:abstractNumId").and_then(|a| a.attr("w:val"));
            let Some((name, levels)) = abs.and_then(|abs| abstracts.get(abs)) else {
                continue;
            };
            let id = match name {
                Some(name) if named.insert(*name) => (*name).to_owned(),
                _ => format!("num{num_id}"),
            };
            map.num_of.insert(id.clone(), num_id.to_owned());
            let def = NumberingDef {
                id,
                levels: levels.clone(),
            };
            map.by_num.insert(num_id.to_owned(), def);
        }
        map
    }

    /// Whether level `level` of `num_id` is ordered, and the definition id a
    /// list item names (none for the built-in lists).
    pub(super) fn list(&self, num_id: &str, level: u8) -> (bool, Option<String>) {
        let Some(def) = self.by_num.get(num_id) else {
            return (false, None);
        };
        let format = def
            .levels
            .get(usize::from(level))
            .or_else(|| def.levels.first())
            .map_or(NumberFormat::Bullet, |l| l.format);
        let ordered = format != NumberFormat::Bullet;
        let builtin = def.id == BUILTIN_BULLET || def.id == BUILTIN_DECIMAL;
        (ordered, (!builtin).then(|| def.id.clone()))
    }

    /// Every definition but the built-in lists, in `numId` order.
    pub(super) fn defs(&self) -> Vec<NumberingDef> {
        let mut nums: Vec<(&String, &NumberingDef)> = self.by_num.iter().collect();
        nums.sort_by_key(|(num, _)| num.parse::<u32>().unwrap_or(u32::MAX));
        nums.into_iter()
            .map(|(_, def)| def)
            .filter(|def| def.id != BUILTIN_BULLET && def.id != BUILTIN_DECIMAL)
            .cloned()
            .collect()
    }
}

fn level(lvl: &Element) -> NumberingLevel {
    let val = |name: &str| lvl.child(name).and_then(|e| e.attr("w:val"));
    NumberingLevel {
        format: NumberFormat::from_ooxml(val("w:numFmt").unwrap_or("bullet")),
        text: val("w:lvlText").unwrap_or_default().to_owned(),
        start: val("w:start").and_then(|s| s.parse().ok()).unwrap_or(1),
        indent: lvl
            .child("w:pPr")
            .and_then(|p| p.child("w:ind"))
            .and_then(|i| i.attr("w:left").or_else(|| i.attr("w:start")))
            .and_then(points),
    }
}
