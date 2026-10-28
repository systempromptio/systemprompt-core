//! Inline text with marks: collecting it while parsing, and writing it as
//! Markdown or HTML.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use super::rich_text::{Mark, MarkKind};

#[derive(Debug, Default)]
pub(crate) struct InlineBuf {
    text: String,
    len: usize,
    open: Vec<(MarkKind, usize)>,
    marks: Vec<Mark>,
}

impl InlineBuf {
    pub(crate) fn push(&mut self, text: &str) {
        self.text.push_str(text);
        self.len += text.chars().count();
    }

    pub(crate) fn open(&mut self, kind: MarkKind) {
        self.open.push((kind, self.len));
    }

    pub(crate) fn close_key(&mut self, key: &str) {
        if let Some(at) = self.open.iter().rposition(|(kind, _)| kind.key() == key) {
            let (kind, start) = self.open.remove(at);
            self.close(kind, start);
        }
    }

    fn close(&mut self, kind: MarkKind, start: usize) {
        if start < self.len {
            self.marks.push(Mark {
                start,
                end: self.len,
                kind,
            });
        }
    }

    pub(crate) fn take(&mut self) -> (String, Vec<Mark>) {
        for (kind, start) in std::mem::take(&mut self.open) {
            self.close(kind, start);
        }
        let text = std::mem::take(&mut self.text);
        let mut marks = std::mem::take(&mut self.marks);
        marks.sort_by(|a, b| (a.start, a.end, &a.kind).cmp(&(b.start, b.end, &b.kind)));
        self.len = 0;
        (text, marks)
    }
}

pub fn segments<'a>(text: &'a str, marks: &'a [Mark]) -> Vec<(String, Vec<&'a MarkKind>)> {
    let chars: Vec<char> = text.chars().collect();
    let mut cuts: BTreeSet<usize> = BTreeSet::from([0, chars.len()]);
    for mark in marks {
        cuts.insert(mark.start.min(chars.len()));
        cuts.insert(mark.end.min(chars.len()));
    }
    let cuts: Vec<usize> = cuts.into_iter().collect();
    cuts.windows(2)
        .filter(|w| w[0] < w[1])
        .map(|w| {
            let mut active: Vec<&MarkKind> = marks
                .iter()
                .filter(|m| m.start <= w[0] && m.end >= w[1])
                .map(|m| &m.kind)
                .collect();
            active.sort();
            active.dedup();
            (chars[w[0]..w[1]].iter().collect(), active)
        })
        .collect()
}

pub(crate) fn render_markdown(text: &str, marks: &[Mark]) -> String {
    let mut out = String::new();
    for (run, active) in segments(text, marks) {
        let has = |key: &str| active.iter().any(|kind| kind.key() == key);
        let mut piece = if has("code") {
            format!("`{run}`")
        } else {
            escape_markdown(&run)
        };
        for (key, fence) in [("strike", "~~"), ("italic", "*"), ("bold", "**")] {
            if has(key) {
                piece = format!("{fence}{piece}{fence}");
            }
        }
        if let Some(MarkKind::Link { href }) = active.iter().find(|kind| kind.key() == "link") {
            piece = format!("[{piece}]({href})");
        }
        out.push_str(&piece);
    }
    out
}

fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '*' | '_' | '`' | '[' | ']' | '~') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

pub(crate) fn render_html(text: &str, marks: &[Mark]) -> String {
    let mut out = String::new();
    for (run, active) in segments(text, marks) {
        let mut piece = escape_html(&run).replace('\n', "<br>");
        for kind in active.iter().rev() {
            piece = match kind {
                MarkKind::Bold => format!("<strong>{piece}</strong>"),
                MarkKind::Italic => format!("<em>{piece}</em>"),
                MarkKind::Underline => format!("<u>{piece}</u>"),
                MarkKind::Strike => format!("<s>{piece}</s>"),
                MarkKind::Code => format!("<code>{piece}</code>"),
                MarkKind::Link { href } => format!("<a href=\"{}\">{piece}</a>", escape_html(href)),
                MarkKind::Color { value } => {
                    format!(
                        "<span style=\"color:{}\">{piece}</span>",
                        escape_html(value)
                    )
                },
                MarkKind::Highlight { value } => {
                    format!(
                        "<mark style=\"background:{}\">{piece}</mark>",
                        escape_html(value)
                    )
                },
                MarkKind::Comment { thread } => {
                    format!(
                        "<span data-comment=\"{}\">{piece}</span>",
                        escape_html(thread)
                    )
                },
                MarkKind::Font { value } => format!(
                    "<span style=\"font-family:{}\">{piece}</span>",
                    escape_html(value)
                ),
                MarkKind::Size { value } => format!(
                    "<span style=\"font-size:{}pt\">{piece}</span>",
                    escape_html(value)
                ),
                MarkKind::Superscript => format!("<sup>{piece}</sup>"),
                MarkKind::Subscript => format!("<sub>{piece}</sub>"),
                MarkKind::Caps => {
                    format!("<span style=\"text-transform:uppercase\">{piece}</span>")
                },
                MarkKind::Field { code } => {
                    format!("<span data-field=\"{}\">{piece}</span>", escape_html(code))
                },
            };
        }
        out.push_str(&piece);
    }
    out
}

pub(crate) fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
