//! Same-turn reuse: a tool-result round trip sends the block its turn
//! already sent. The stored text is parsed back into a [`RecallBlock`] and
//! accepted only when it renders to the identical bytes, so the prompt
//! cache stays warm and nothing drifts mid-task. The stored text carries no
//! revisions, so a reused fact reports revision 0.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_extension::brain::{InjectedFact, RecallBlock};
use systemprompt_identifiers::BrainFactId;

const OPEN: &str = "<team_memory scope=\"";
const AS_OF: &str = "\" as_of=\"";
const CLOSE: &str = "</team_memory>";
const SEPARATOR: &str = " · ";

#[must_use]
pub fn parse_stored(stored: &str) -> Option<RecallBlock> {
    let mut lines = stored.lines();
    let header = lines.next()?.strip_prefix(OPEN)?.strip_suffix("\">")?;
    let (scope, as_of) = header.split_once(AS_OF)?;
    let as_of: DateTime<Utc> = DateTime::parse_from_rfc3339(as_of)
        .ok()?
        .with_timezone(&Utc);
    let mut block = RecallBlock::new(unescape(scope), as_of);
    for line in lines {
        if line == CLOSE {
            break;
        }
        block = block.with_fact(parse_line(line)?);
    }
    (block.render() == stored).then_some(block)
}

fn parse_line(line: &str) -> Option<InjectedFact> {
    let rest = line.strip_prefix("- [")?;
    let (id, rest) = rest.split_once(SEPARATOR)?;
    let (source, text) = rest.split_once("] ")?;
    Some(InjectedFact {
        id: BrainFactId::try_new(id).ok()?,
        revision: 0,
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
