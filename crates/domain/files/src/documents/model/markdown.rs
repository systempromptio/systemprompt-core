//! Markdown to and from the `RichText` model.
//!
//! [`import`] parses `CommonMark` with tables and strikethrough into blocks
//! with deterministic ids (`b1`, `b2`, …), so every replica seeding the same
//! revision builds the same model. [`materialise`] writes the model back as
//! Markdown; marks Markdown cannot express (underline, colour, highlight,
//! comments) are dropped from the text, since comments live in the
//! document's comment store and are re-anchored by id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::ids::IdSeq;
use super::inline::{InlineBuf, render_markdown};
use super::rich_text::{Block, BlockKind, MarkKind, RichText};

#[must_use]
pub fn import(markdown: &str) -> RichText {
    let mut builder = Builder::default();
    for event in Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    ) {
        builder.event(event);
    }
    RichText {
        blocks: builder.blocks,
    }
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    ids: IdSeq,
    inline: InlineBuf,
    kind: Option<BlockKind>,
    lists: Vec<bool>,
    quote: usize,
    rows: Vec<Vec<RichText>>,
    table: bool,
}

impl Builder {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                self.inline.push(&text)
            },
            Event::Code(text) => {
                self.inline.open(MarkKind::Code);
                self.inline.push(&text);
                self.inline.close_key("code");
            },
            Event::SoftBreak => self.inline.push(" "),
            Event::HardBreak => self.inline.push("\n"),
            Event::Rule => self.emit(BlockKind::Divider),
            _ => {},
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.kind = Some(BlockKind::Heading {
                    level: heading(level),
                })
            },
            Tag::Paragraph if self.kind.is_none() => {
                self.kind = Some(if self.quote > 0 {
                    BlockKind::Quote
                } else {
                    BlockKind::Paragraph
                });
            },
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.to_string()),
                    _ => None,
                };
                self.kind = Some(BlockKind::Code { language });
            },
            Tag::BlockQuote(_) => self.quote += 1,
            Tag::List(first) => self.lists.push(first.is_some()),
            Tag::Item => {
                let level = u8::try_from(self.lists.len().saturating_sub(1)).unwrap_or(u8::MAX);
                let ordered = self.lists.last().copied().unwrap_or(false);
                self.kind = Some(BlockKind::ListItem { level, ordered });
            },
            Tag::Table(_) => self.table = true,
            Tag::TableRow | Tag::TableHead => self.rows.push(Vec::new()),
            Tag::Emphasis => self.inline.open(MarkKind::Italic),
            Tag::Strong => self.inline.open(MarkKind::Bold),
            Tag::Strikethrough => self.inline.open(MarkKind::Strike),
            Tag::Link { dest_url, .. } => self.inline.open(MarkKind::Link {
                href: dest_url.to_string(),
            }),
            Tag::Image { dest_url, .. } => {
                self.flush();
                self.kind = Some(BlockKind::Image {
                    blob: dest_url.to_string(),
                    alt: String::new(),
                });
            },
            _ => {},
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::CodeBlock | TagEnd::Item => {
                self.flush()
            },
            TagEnd::Image => {
                let alt = self.inline.take().0;
                if let Some(BlockKind::Image { blob, .. }) = self.kind.take() {
                    self.emit(BlockKind::Image { blob, alt });
                }
            },
            TagEnd::BlockQuote(_) => self.quote = self.quote.saturating_sub(1),
            TagEnd::List(_) => {
                self.lists.pop();
            },
            TagEnd::TableCell => {
                let (text, marks) = self.inline.take();
                let mut cell = Block::new(self.ids.next_id(), BlockKind::Paragraph, text);
                cell.marks = marks;
                if let Some(row) = self.rows.last_mut() {
                    row.push(RichText { blocks: vec![cell] });
                }
            },
            TagEnd::Table => {
                self.table = false;
                let rows = std::mem::take(&mut self.rows);
                self.emit(BlockKind::Table { rows });
            },
            TagEnd::Emphasis => self.inline.close_key("italic"),
            TagEnd::Strong => self.inline.close_key("bold"),
            TagEnd::Strikethrough => self.inline.close_key("strike"),
            TagEnd::Link => self.inline.close_key("link"),
            _ => {},
        }
    }

    fn flush(&mut self) {
        let Some(kind) = self.kind.take() else {
            return;
        };
        let (mut text, marks) = self.inline.take();
        if matches!(kind, BlockKind::Code { .. }) && text.ends_with('\n') {
            text.pop();
        }
        if self.table {
            return;
        }
        let mut block = Block::new(self.ids.next_id(), kind, text);
        block.marks = marks;
        self.blocks.push(block);
    }

    fn emit(&mut self, kind: BlockKind) {
        self.blocks
            .push(Block::new(self.ids.next_id(), kind, String::new()));
    }
}

const fn heading(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[must_use]
pub fn materialise(model: &RichText) -> String {
    let mut out = String::new();
    let mut previous_list = false;
    for block in &model.blocks {
        let is_list = matches!(block.kind, BlockKind::ListItem { .. });
        if !out.is_empty() {
            out.push_str(if is_list && previous_list {
                "\n"
            } else {
                "\n\n"
            });
        }
        previous_list = is_list;
        write_block(&mut out, block);
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn write_block(out: &mut String, block: &Block) {
    let inline = || render_markdown(&block.text, &block.marks);
    match &block.kind {
        BlockKind::Paragraph => out.push_str(&inline()),
        BlockKind::Heading { level } => {
            out.push_str(&"#".repeat(usize::from(*level)));
            out.push(' ');
            out.push_str(&inline());
        },
        BlockKind::ListItem { level, ordered } => {
            out.push_str(&"  ".repeat(usize::from(*level)));
            out.push_str(if *ordered { "1. " } else { "- " });
            out.push_str(&inline());
        },
        BlockKind::Quote => {
            out.push_str("> ");
            out.push_str(&inline().replace('\n', "\n> "));
        },
        BlockKind::Code { language } => {
            out.push_str("```");
            out.push_str(language.as_deref().unwrap_or(""));
            out.push('\n');
            out.push_str(&block.text);
            out.push_str("\n```");
        },
        BlockKind::Table { rows } => write_table(out, rows),
        BlockKind::Image { blob, alt } => out.push_str(&format!("![{alt}]({blob})")),
        BlockKind::Divider | BlockKind::PageBreak => out.push_str("---"),
        BlockKind::Raw { source } => out.push_str(source),
    }
}

fn write_table(out: &mut String, rows: &[Vec<RichText>]) {
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    for (index, row) in rows.iter().enumerate() {
        out.push('|');
        for column in 0..width {
            let cell = row.get(column).map(cell_text).unwrap_or_default();
            out.push_str(&format!(" {} |", cell.replace('|', "\\|")));
        }
        if index == 0 {
            out.push_str("\n|");
            out.push_str(&" --- |".repeat(width));
        }
        if index + 1 < rows.len() {
            out.push('\n');
        }
    }
}

fn cell_text(cell: &RichText) -> String {
    cell.blocks
        .iter()
        .map(|block| render_markdown(&block.text, &block.marks))
        .collect::<Vec<_>>()
        .join(" ")
}
