//! The matching ladder behind edits: an exact match first, then progressively
//! more forgiving about whitespace. The first rung that finds anything decides,
//! so a looser rung never overrides a stricter one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Strategy {
    Exact,
    IndentFlexible,
    LineTrimmed,
    WhitespaceNormalized,
}

impl Strategy {
    const LADDER: [Self; 4] = [
        Self::Exact,
        Self::IndentFlexible,
        Self::LineTrimmed,
        Self::WhitespaceNormalized,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::IndentFlexible => "indentation-flexible",
            Self::LineTrimmed => "line-trimmed",
            Self::WhitespaceNormalized => "whitespace-normalized",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MatchError {
    EmptyNeedle,
    NotFound,
    Ambiguous(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Replaced {
    pub(crate) text: String,
    pub(crate) strategy: Strategy,
    pub(crate) count: usize,
}

pub(crate) fn replace(
    content: &str,
    old: &str,
    new: &str,
    replace_all: bool,
) -> Result<Replaced, MatchError> {
    if old.is_empty() {
        return Err(MatchError::EmptyNeedle);
    }
    for strategy in Strategy::LADDER {
        let spans = find(content, old, strategy);
        if spans.is_empty() {
            continue;
        }
        if spans.len() > 1 && !replace_all {
            return Err(MatchError::Ambiguous(spans.len()));
        }
        let mut out = String::with_capacity(content.len() + new.len());
        let mut last = 0;
        for span in &spans {
            out.push_str(&content[last..span.start]);
            if strategy == Strategy::IndentFlexible {
                out.push_str(&reindent(new, old, &content[span.clone()]));
            } else {
                out.push_str(new);
            }
            last = span.end;
        }
        out.push_str(&content[last..]);
        return Ok(Replaced {
            text: out,
            strategy,
            count: spans.len(),
        });
    }
    Err(MatchError::NotFound)
}

pub(crate) fn find(content: &str, needle: &str, strategy: Strategy) -> Vec<Range<usize>> {
    match strategy {
        Strategy::Exact => content
            .match_indices(needle)
            .map(|(at, hit)| at..at + hit.len())
            .collect(),
        Strategy::IndentFlexible => {
            let target = dedent(&needle_lines(needle));
            blocks(content, needle, |window, _| dedent(window) == target)
        },
        Strategy::LineTrimmed => blocks(content, needle, |window, want| {
            window
                .iter()
                .zip(want)
                .all(|(have, want)| have.trim() == want.trim())
        }),
        Strategy::WhitespaceNormalized => blocks(content, needle, |window, want| {
            window
                .iter()
                .zip(want)
                .all(|(have, want)| squash(have) == squash(want))
        }),
    }
}

pub(crate) fn squash(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn line_spans(content: &str) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut start = 0;
    for (at, _) in content.match_indices('\n') {
        spans.push(start..at);
        start = at + 1;
    }
    if start < content.len() {
        spans.push(start..content.len());
    }
    spans
}

fn needle_lines(needle: &str) -> Vec<&str> {
    needle
        .strip_suffix('\n')
        .unwrap_or(needle)
        .split('\n')
        .collect()
}

fn blocks<F>(content: &str, needle: &str, same: F) -> Vec<Range<usize>>
where
    F: Fn(&[&str], &[&str]) -> bool,
{
    let want = needle_lines(needle);
    if want.iter().all(|line| line.trim().is_empty()) {
        return Vec::new();
    }
    let spans = line_spans(content);
    let lines: Vec<&str> = spans.iter().map(|span| &content[span.clone()]).collect();
    let size = want.len();
    let mut out = Vec::new();
    let mut at = 0;
    while at + size <= lines.len() {
        if same(&lines[at..at + size], &want) {
            let start = spans[at].start;
            let mut end = spans[at + size - 1].end;
            if needle.ends_with('\n') && content[end..].starts_with('\n') {
                end += 1;
            }
            out.push(start..end);
            at += size;
        } else {
            at += 1;
        }
    }
    out
}

fn indent_width(line: &str) -> usize {
    line.len() - line.trim_start_matches([' ', '\t']).len()
}

fn dedent(lines: &[&str]) -> Vec<String> {
    let min = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| indent_width(line))
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                line[min..].trim_end().to_owned()
            }
        })
        .collect()
}

fn common_indent(text: &str) -> &str {
    text.split('\n')
        .filter(|line| !line.trim().is_empty())
        .map(|line| &line[..indent_width(line)])
        .min_by_key(|indent| indent.len())
        .unwrap_or("")
}

fn reindent(new: &str, old: &str, matched: &str) -> String {
    let from = common_indent(old);
    let to = common_indent(matched);
    if from == to {
        return new.to_owned();
    }
    new.split('\n')
        .map(|line| {
            if line.trim().is_empty() {
                line.to_owned()
            } else {
                let rest = line.strip_prefix(from).unwrap_or_else(|| line.trim_start());
                format!("{to}{rest}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
