//! Rewrites the paragraphs of a paired text shape whose marks or kind the
//! target changed, once the text passes have made their text agree.
//!
//! A paragraph is rewritten only when its [`segments`] or its list-ness
//! differ from the target's, so an untouched paragraph keeps its bytes. The
//! paragraph keeps its `a:pPr` (a bullet added or taken out when it became
//! or stopped being a list item) and its end-of-paragraph properties, and its
//! runs are rebuilt from the first run's properties with the target's marks.
//! A deck has no headings: any non-list kind is a plain paragraph.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::CodecError;

use super::paragraph::without_bullets;
use super::runs::{runs_xml, segments};
use crate::documents::codecs::xml_tree::{Element, parse_xml, splice};
use crate::documents::engine::loro::deck::DeckShape;
use crate::documents::model::{Block, BlockKind};

const BULLETS: [&str; 4] = ["a:buNone", "a:buChar", "a:buAutoNum", "a:buBlip"];
const BULLET_CHAR: &str = "<a:buChar char=\"\u{2022}\"/>";

fn listed(block: &Block) -> bool {
    matches!(block.kind, BlockKind::ListItem { .. })
}

fn with_bullet(ppr: Option<&str>) -> Result<String, CodecError> {
    let Some(raw) = ppr else {
        let font = "<a:buFont typeface=\"Arial\"/>";
        return Ok(format!(
            "<a:pPr marL=\"342900\" indent=\"-342900\">{font}{BULLET_CHAR}</a:pPr>"
        ));
    };
    let root = parse_xml("a:pPr", raw)?;
    if root.inner_start == root.end {
        let open = raw.trim_end().trim_end_matches("/>");
        return Ok(format!("{open}>{BULLET_CHAR}</a:pPr>"));
    }
    let mut edits: Vec<(usize, usize, String)> = root
        .children
        .iter()
        .filter(|child| BULLETS.contains(&child.name.as_str()))
        .map(|child| (child.start, child.end, String::new()))
        .collect();
    let at = edits.first().map_or_else(
        || {
            root.children
                .iter()
                .find(|child| matches!(child.name.as_str(), "a:tabLst" | "a:defRPr" | "a:extLst"))
                .map_or(root.inner_end, |child| child.start)
        },
        |edit| edit.0,
    );
    edits.push((at, at, BULLET_CHAR.to_owned()));
    Ok(splice(raw, edits))
}

fn paragraph_xml(
    xml: &str,
    p: &Element,
    base: &Block,
    target: &Block,
) -> Result<String, CodecError> {
    let ppr = p.child("a:pPr").map(|el| el.raw(xml));
    let ppr = match (listed(base), listed(target)) {
        (false, true) => with_bullet(ppr)?,
        (true, false) => without_bullets(ppr)?,
        _ => ppr.unwrap_or_default().to_owned(),
    };
    let rpr = p
        .children
        .iter()
        .filter(|child| matches!(child.name.as_str(), "a:r" | "a:fld"))
        .find_map(|run| run.child("a:rPr"))
        .map(|el| el.raw(xml));
    let runs = runs_xml(&segments(&target.text, &target.marks), rpr);
    let end = p.child("a:endParaRPr").map_or("", |el| el.raw(xml));
    Ok(format!("<a:p>{ppr}{runs}{end}</a:p>"))
}

pub(super) fn paragraph_edits(
    xml: &str,
    element: &Element,
    base: &DeckShape,
    target: &DeckShape,
) -> Result<Vec<(usize, usize, String)>, CodecError> {
    let paragraphs: Vec<&Element> = element
        .child("p:txBody")
        .map_or_else(Vec::new, |body| body.children_named("a:p").collect());
    if !base.kind.holds_text()
        || paragraphs.len() != base.paragraphs.len()
        || base.paragraphs.len() != target.paragraphs.len()
    {
        return Ok(Vec::new());
    }
    let mut edits = Vec::new();
    let pairs = paragraphs
        .iter()
        .zip(&base.paragraphs)
        .zip(&target.paragraphs);
    for ((p, base), target) in pairs {
        if base.text != target.text {
            continue;
        }
        let restyled = segments(&base.text, &base.marks) != segments(&target.text, &target.marks);
        if restyled || listed(base) != listed(target) {
            edits.push((p.start, p.end, paragraph_xml(xml, p, base, target)?));
        }
    }
    Ok(edits)
}
