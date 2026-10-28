//! Markdown to and from the `RichText` model.
//!
//! [`import`] parses GitHub-flavoured Markdown (tables, strikethrough, task
//! lists, footnotes, alerts and YAML front matter) into blocks with
//! deterministic ids (`b1`, `b2`, …), so every replica seeding the same
//! revision builds the same model. A fenced code block keeps its language,
//! an image its title as the caption, an alert (`> [!NOTE]`) becomes a
//! callout, and the inline HTML tags `u`, `sup` and `sub` become marks.
//! [`materialise`] (in `markdown_write`) writes the model back and refuses
//! what Markdown cannot hold rather than dropping it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use super::document::DocumentSettings;
use super::ids::IdSeq;
use super::inline::InlineBuf;
use super::layout::CalloutTone;
use super::markdown_tags::{INLINE_TAGS, heading, tone};
use super::rich_text::{Block, BlockKind, MarkKind, RichText};

pub use super::markdown_degrade::degrade;
pub use super::markdown_write::materialise;

const OPTIONS: Options = Options::ENABLE_TABLES
    .union(Options::ENABLE_STRIKETHROUGH)
    .union(Options::ENABLE_TASKLISTS)
    .union(Options::ENABLE_FOOTNOTES)
    .union(Options::ENABLE_GFM)
    .union(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);


#[must_use]
pub fn import(markdown: &str) -> RichText {
    let mut builder = Builder::default();
    for event in Parser::new_ext(markdown, OPTIONS) {
        builder.event(event);
    }
    let mut model = RichText::of(builder.blocks);
    if let Some(front_matter) = builder.front_matter {
        model.document = Some(Box::new(DocumentSettings {
            front_matter: Some(front_matter),
            ..DocumentSettings::default()
        }));
    }
    model
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Block>,
    ids: IdSeq,
    inline: InlineBuf,
    kind: Option<BlockKind>,
    lists: Vec<bool>,
    quote: usize,
    callout: Option<CalloutTone>,
    footnote: Option<String>,
    rows: Vec<Vec<RichText>>,
    table: bool,
    caption: Option<String>,
    metadata: bool,
    front_matter: Option<String>,
}

impl Builder {
    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) if self.metadata => {
                self.front_matter
                    .get_or_insert_with(String::new)
                    .push_str(&text);
            },
            Event::InlineHtml(html) => self.inline_html(&html),
            Event::Text(text) | Event::Html(text) => self.inline.push(&text),
            Event::Code(text) => {
                self.inline.open(MarkKind::Code);
                self.inline.push(&text);
                self.inline.close_key("code");
            },
            Event::FootnoteReference(label) => {
                self.inline.open(MarkKind::Footnote {
                    label: label.to_string(),
                });
                self.inline.push(&label);
                self.inline.close_key("footnote");
            },
            Event::TaskListMarker(ticked) => {
                if let Some(BlockKind::ListItem { checked, .. }) = self.kind.as_mut() {
                    *checked = Some(ticked);
                }
            },
            Event::SoftBreak => self.inline.push(" "),
            Event::HardBreak => self.inline.push("\n"),
            Event::Rule => self.emit(BlockKind::Divider),
            _ => {},
        }
    }

    fn inline_html(&mut self, html: &str) {
        let tag = html.trim().trim_start_matches('<').trim_end_matches('>');
        let (closing, name) = tag
            .strip_prefix('/')
            .map_or((false, tag), |name| (true, name));
        match INLINE_TAGS.iter().find(|(html, _)| *html == name) {
            Some((_, key)) if closing => self.inline.close_key(key),
            Some((_, key)) => {
                if let Some(kind) = MarkKind::from_key(key, "") {
                    self.inline.open(kind);
                }
            },
            None => self.inline.push(html),
        }
    }

    fn text_kind(&self) -> BlockKind {
        if let Some(label) = &self.footnote {
            return BlockKind::Footnote {
                label: label.clone(),
            };
        }
        match (self.callout, self.quote) {
            (Some(tone), _) => BlockKind::Callout { tone },
            (None, 0) => BlockKind::Paragraph,
            (None, _) => BlockKind::Quote,
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.kind = Some(BlockKind::Heading {
                    level: heading(level),
                });
            },
            Tag::Paragraph if self.kind.is_none() => self.kind = Some(self.text_kind()),
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.to_string()),
                    _ => None,
                };
                self.kind = Some(BlockKind::Code { language });
            },
            Tag::BlockQuote(kind) => {
                self.quote += 1;
                self.callout = kind.map(tone).or(self.callout);
            },
            Tag::FootnoteDefinition(label) => self.footnote = Some(label.to_string()),
            Tag::MetadataBlock(_) => self.metadata = true,
            Tag::List(first) => {
                if matches!(self.kind, Some(BlockKind::ListItem { .. })) {
                    self.flush();
                }
                self.lists.push(first.is_some());
            },
            Tag::Item => {
                let level = u8::try_from(self.lists.len().saturating_sub(1)).unwrap_or(u8::MAX);
                let ordered = self.lists.last().copied().unwrap_or(false);
                self.kind = Some(BlockKind::list_item(level, ordered));
            },
            Tag::Table(_) => self.table = true,
            Tag::TableRow | Tag::TableHead => self.rows.push(Vec::new()),
            Tag::Emphasis => self.inline.open(MarkKind::Italic),
            Tag::Strong => self.inline.open(MarkKind::Bold),
            Tag::Strikethrough => self.inline.open(MarkKind::Strike),
            Tag::Link { dest_url, .. } => self.inline.open(MarkKind::Link {
                href: dest_url.to_string(),
            }),
            Tag::Image {
                dest_url, title, ..
            } => {
                self.flush();
                self.caption = (!title.is_empty()).then(|| title.to_string());
                self.kind = Some(BlockKind::image(dest_url.to_string(), String::new()));
            },
            _ => {},
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) | TagEnd::Paragraph | TagEnd::CodeBlock | TagEnd::Item => {
                self.flush();
            },
            TagEnd::Image => self.end_image(),
            TagEnd::BlockQuote(_) => {
                self.quote = self.quote.saturating_sub(1);
                if self.quote == 0 {
                    self.callout = None;
                }
            },
            TagEnd::FootnoteDefinition => self.footnote = None,
            TagEnd::MetadataBlock(_) => self.metadata = false,
            TagEnd::List(_) => {
                self.lists.pop();
            },
            TagEnd::TableCell => {
                let (text, marks) = self.inline.take();
                let mut cell = Block::new(self.ids.next_id(), BlockKind::Paragraph, text);
                cell.marks = marks;
                if let Some(row) = self.rows.last_mut() {
                    row.push(RichText::of(vec![cell]));
                }
            },
            TagEnd::Table => {
                self.table = false;
                let rows = std::mem::take(&mut self.rows);
                let mut kind = BlockKind::table(rows);
                if let BlockKind::Table { props, .. } = &mut kind {
                    props.header_rows = 1;
                }
                self.emit(kind);
            },
            TagEnd::Emphasis => self.inline.close_key("italic"),
            TagEnd::Strong => self.inline.close_key("bold"),
            TagEnd::Strikethrough => self.inline.close_key("strike"),
            TagEnd::Link => self.inline.close_key("link"),
            _ => {},
        }
    }

    fn end_image(&mut self) {
        let alt = self.inline.take().0;
        if let Some(BlockKind::Image { blob, .. }) = self.kind.take() {
            let mut kind = BlockKind::image(blob, alt);
            if let BlockKind::Image { caption, .. } = &mut kind {
                *caption = self.caption.take();
            }
            self.emit(kind);
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
