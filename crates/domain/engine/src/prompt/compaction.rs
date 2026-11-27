//! Compaction: condensing a long session into a structured summary that
//! replaces its history.
//!
//! The compaction agent runs with
//! [`COMPACTION_PROMPT`](crate::prompt::compaction::COMPACTION_PROMPT) as its system
//! prompt, the session's messages, and [`request`](crate::prompt::compaction::request)
//! as the final user message. Its reply becomes the summary;
//! [`summary_message`](crate::prompt::compaction::summary_message) frames it at the
//! start of the new history and
//! [`CONTINUE_AFTER_COMPACTION`](crate::prompt::compaction::CONTINUE_AFTER_COMPACTION)
//! follows an automatic compaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const COMPACTION_PROMPT: &str = r"You condense a coding session so that it can carry on in a fresh context. Your summary replaces the conversation: anything it leaves out is lost to the agent that picks the work up. Write it for that agent, not for the user.
Be specific. Keep exact file paths, function and type names, commands, error messages, versions, numbers, and the user's own words where they matter. Drop pleasantries, raw tool output and dead ends that taught nothing, but keep the conclusion each investigation reached.
Do not call tools and do not continue the task. Reply with the summary alone, in the format you are asked for.";

pub const SECTIONS: &[&str] = &[
    "Goal",
    "Constraints and preferences",
    "Progress",
    "Key decisions",
    "Relevant files",
    "Next steps",
];

const SECTION_TEMPLATE: &str = r"## Goal
What the user is trying to achieve overall, in one to three sentences.

## Constraints and preferences
Requirements, rules and preferences the user stated or the project imposes: conventions, things to avoid, how they want to work.

## Progress
### Done
Work that is finished and verified.
### In progress
Work that is started but not finished, with its exact state.
### Blocked
Anything waiting on the user, a failing check or missing information, and what it waits for.

## Key decisions
Choices that were made and the reason for each, including approaches that were tried and rejected.

## Relevant files
Each file that matters, as a path with one line on its role and what was changed in it.

## Next steps
The concrete next actions, in order.";

pub fn request(previous_summary: Option<&str>) -> String {
    previous_summary.map(str::trim).filter(|summary| !summary.is_empty()).map_or_else(
        || format!(
            "Summarise the session so far. Use exactly these sections, in this order, as Markdown headings, and write \"None\" under any section that has nothing in it.\n\n{SECTION_TEMPLATE}"
        ),
        |previous| format!(
            "An earlier part of this session was already summarised:\n\n<previous-summary>\n{previous}\n</previous-summary>\n\nUpdate that summary with everything that has happened since. Keep what is still true, correct what has changed, move finished work from In progress to Done, and remove only what no longer matters. Use exactly the same sections, in this order, as Markdown headings, and write \"None\" under any section that has nothing in it.\n\n{SECTION_TEMPLATE}"
        ),
    )
}

pub fn summary_message(summary: &str) -> String {
    format!(
        "Earlier messages in this session were compacted into the summary below. Treat it as the record of everything that happened before this point.\n\n{}",
        summary.trim()
    )
}

pub const CONTINUE_AFTER_COMPACTION: &str = "The session was compacted to free up context; the summary above holds everything known so far. If there are next steps, carry on with them now. If the task is finished or you need the user's input, say so briefly instead of starting new work.";
