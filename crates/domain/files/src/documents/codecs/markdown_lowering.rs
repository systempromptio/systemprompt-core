//! Markdown lowered into a format-neutral block list that each codec
//! writes as native markup.
//!
//! [`lower`] parses `CommonMark` with tables and strikethrough. Lists are
//! flattened into [`Block::ListItem`]s that carry their nesting depth and,
//! for ordered lists, their number; inline formatting is flattened into
//! [`Span`]s carrying bold, italic, code and link flags. Block quotes
//! contribute their paragraphs, images their alt text and raw HTML its
//! source text; rules are dropped. A `<!-- pending: intent -->` line is a
//! [`Block::Pending`] placeholder. A codec that cannot represent a block
//! natively writes [`Block::plain`] as a paragraph, so Markdown is never
//! refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use super::pending::{intent_of_line, text_marker};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Block {
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Paragraph(Vec<Span>),
    ListItem {
        depth: usize,
        number: Option<u64>,
        spans: Vec<Span>,
    },
    Table(Vec<Vec<Vec<Span>>>),
    Code(String),
    Pending(String),
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    spans: Vec<Span>,
    bold: usize,
    italic: usize,
    links: Vec<String>,
    lists: Vec<Option<u64>>,
    in_item: bool,
    rows: Vec<Vec<Vec<Span>>>,
    row: Vec<Vec<Span>>,
    code: Option<String>,
}

pub(crate) fn lower(markdown: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut chunk = String::new();
    for line in markdown.lines() {
        if let Some(intent) = intent_of_line(line) {
            blocks.extend(lower_chunk(&chunk));
            chunk.clear();
            blocks.push(Block::Pending(intent.to_owned()));
        } else {
            chunk.push_str(line);
            chunk.push('\n');
        }
    }
    blocks.extend(lower_chunk(&chunk));
    blocks
}

fn lower_chunk(markdown: &str) -> Vec<Block> {
    let mut builder = Builder::default();
    for event in Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    ) {
        builder.event(event);
    }
    builder.flush_item();
    builder.blocks
}

impl Builder {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                if let Some(code) = self.code.as_mut() {
                    code.push_str(&text);
                } else {
                    self.push(&text, false);
                }
            },
            Event::Code(text) => self.push(&text, true),
            Event::SoftBreak => self.push(" ", false),
            Event::HardBreak => self.push("\n", false),
            _ => {},
        }
    }

    fn push(&mut self, text: &str, code: bool) {
        self.spans.push(Span {
            text: text.to_owned(),
            bold: self.bold > 0,
            italic: self.italic > 0,
            code,
            link: self.links.last().cloned(),
        });
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Strong => self.bold += 1,
            Tag::Emphasis => self.italic += 1,
            Tag::Link { dest_url, .. } => self.links.push(dest_url.into_string()),
            Tag::List(start) => {
                self.flush_item();
                self.lists.push(start);
            },
            Tag::Item => {
                self.flush_item();
                self.in_item = true;
            },
            Tag::CodeBlock(_) => {
                self.flush_item();
                self.code = Some(String::new());
            },
            Tag::TableHead | Tag::TableRow => self.row.clear(),
            Tag::TableCell => self.spans.clear(),
            _ => {},
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Strong => self.bold = self.bold.saturating_sub(1),
            TagEnd::Emphasis => self.italic = self.italic.saturating_sub(1),
            TagEnd::Link => {
                self.links.pop();
            },
            TagEnd::Heading(level) => {
                let spans = std::mem::take(&mut self.spans);
                self.blocks.push(Block::Heading {
                    level: level as u8,
                    spans,
                });
            },
            TagEnd::Paragraph if !self.in_item => {
                let spans = std::mem::take(&mut self.spans);
                self.blocks.push(Block::Paragraph(spans));
            },
            TagEnd::Paragraph => self.push(" ", false),
            TagEnd::Item => self.flush_item(),
            TagEnd::List(_) => {
                self.flush_item();
                self.lists.pop();
            },
            TagEnd::CodeBlock => {
                if let Some(code) = self.code.take() {
                    self.blocks
                        .push(Block::Code(code.trim_end_matches('\n').to_owned()));
                }
            },
            TagEnd::TableCell => {
                let cell = std::mem::take(&mut self.spans);
                self.row.push(cell);
            },
            TagEnd::TableHead | TagEnd::TableRow => {
                let row = std::mem::take(&mut self.row);
                self.rows.push(row);
            },
            TagEnd::Table => {
                let rows = std::mem::take(&mut self.rows);
                self.blocks.push(Block::Table(rows));
            },
            _ => {},
        }
    }

    fn flush_item(&mut self) {
        if !self.in_item {
            return;
        }
        self.in_item = false;
        let mut spans = std::mem::take(&mut self.spans);
        while spans.last().is_some_and(|span| span.text.trim().is_empty()) {
            spans.pop();
        }
        if spans.is_empty() {
            return;
        }
        let number = self.lists.last_mut().and_then(|next| {
            let number = *next;
            if let Some(n) = next.as_mut() {
                *n += 1;
            }
            number
        });
        self.blocks.push(Block::ListItem {
            depth: self.lists.len().saturating_sub(1),
            number,
            spans,
        });
    }
}

pub(crate) fn plain(spans: &[Span]) -> String {
    spans
        .iter()
        .map(|span| match &span.link {
            Some(url) if *url != span.text => format!("{} ({url})", span.text),
            _ => span.text.clone(),
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

impl Block {
    pub(crate) fn marker(&self) -> String {
        match self {
            Self::ListItem { depth, number, .. } => {
                let indent = "  ".repeat(*depth);
                number.map_or_else(|| format!("{indent}- "), |n| format!("{indent}{n}. "))
            },
            _ => String::new(),
        }
    }

    pub(crate) fn plain(&self) -> String {
        match self {
            Self::Heading { spans, .. } | Self::Paragraph(spans) => plain(spans),
            Self::ListItem { spans, .. } => format!("{}{}", self.marker(), plain(spans)),
            Self::Table(rows) => rows
                .iter()
                .map(|row| row_text(row))
                .collect::<Vec<_>>()
                .join("\n"),
            Self::Code(code) => code.clone(),
            Self::Pending(intent) => text_marker(intent),
        }
    }
}

pub(crate) fn row_text(row: &[Vec<Span>]) -> String {
    row.iter()
        .map(|cell| plain(cell))
        .collect::<Vec<_>>()
        .join(" | ")
}

pub(crate) fn plain_lines(blocks: &[Block]) -> Vec<String> {
    blocks
        .iter()
        .flat_map(|block| {
            block
                .plain()
                .split('\n')
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}
