//! Same-turn reuse.
//!
//! A tool-result round trip sends the block its turn already sent. The
//! stored text is parsed back into a [`RecallBlock`], each fact takes its
//! revision from the stored refs, and the block is accepted only when the
//! refs name the same facts in the same order and it renders to the
//! identical bytes, so the prompt cache stays warm, the memory-ids header
//! keeps its revisions and nothing drifts mid-task.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::brain::{InjectedFact, RecallBlock};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{FactRef, StoredBlock};

const OPEN: &str = "<team_memory scope=\"";
const AS_OF: &str = "\" as_of=\"";
const CLOSE: &str = "</team_memory>";
const SEPARATOR: &str = " · ";

#[must_use]
pub fn parse_stored(stored: &StoredBlock) -> Option<RecallBlock> {
    let mut lines = stored.rendered.lines();
    let header = lines.next()?.strip_prefix(OPEN)?.strip_suffix("\">")?;
    let (scope, _) = header.split_once(AS_OF)?;
    let mut block = RecallBlock::new(unescape(scope), stored.as_of);
    let mut refs = stored.facts.iter();
    for line in lines {
        if line == CLOSE {
            break;
        }
        block = block.with_fact(parse_line(line, refs.next()?)?);
    }
    let complete = refs.next().is_none();
    (complete && block.render() == stored.rendered).then_some(block)
}

fn parse_line(line: &str, fact: &FactRef) -> Option<InjectedFact> {
    let rest = line.strip_prefix("- [")?;
    let (id, rest) = rest.split_once(SEPARATOR)?;
    let (source, text) = rest.split_once("] ")?;
    let id = BrainFactId::try_new(id).ok()?;
    (id == fact.id).then(|| InjectedFact {
        id,
        revision: fact.revision,
        source: unescape(source),
        text: unescape(text),
    })
}

fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
