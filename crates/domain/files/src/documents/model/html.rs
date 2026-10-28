//! HTML bodies to and from the `RichText` model.
//!
//! [`import`] reads the inner markup of `<body>` (the whole input when it has
//! none): headings, paragraphs, list items (nested lists set the level),
//! block quotes, `pre`, tables, images and rules become blocks, and inline
//! `strong`/`b`, `em`/`i`, `u`, `s`/`del`, `code`, `a`, `mark`, coloured
//! `span`s and `span[data-comment]` become marks. Any other element at block
//! level is kept verbatim as a [`BlockKind::Raw`] block, so markup the model
//! does not own survives every checkpoint. [`materialise`] writes the blocks
//! back as markup for the body; the caller splices it into the revision.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use ego_tree::NodeRef;
use scraper::{ElementRef, Html, Node};

use super::ids::IdSeq;
use super::inline::InlineBuf;
use super::rich_text::{Block, BlockKind, MarkKind, RichText};

pub use super::html_write::materialise;

#[must_use]
pub fn import(body: &str) -> RichText {
    let fragment = Html::parse_fragment(body);
    let mut reader = Reader::default();
    reader.children(fragment.root_element(), 0);
    reader.flush_loose();
    RichText {
        blocks: reader.blocks,
    }
}

#[derive(Default)]
struct Reader {
    blocks: Vec<Block>,
    ids: IdSeq,
    loose: InlineBuf,
    loose_any: bool,
}

impl Reader {
    fn children(&mut self, parent: ElementRef<'_>, list_depth: u8) {
        for child in parent.children() {
            match child.value() {
                Node::Element(_) => {
                    if let Some(element) = ElementRef::wrap(child) {
                        self.element(element, list_depth);
                    }
                },
                Node::Text(text) if !text.trim().is_empty() => {
                    self.loose.push(text);
                    self.loose_any = true;
                },
                _ => {},
            }
        }
    }

    fn element(&mut self, element: ElementRef<'_>, list_depth: u8) {
        let name = element.value().name();
        let kind = match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => BlockKind::Heading {
                level: name.as_bytes()[1] - b'0',
            },
            "p" => BlockKind::Paragraph,
            "blockquote" => BlockKind::Quote,
            "pre" => BlockKind::Code { language: None },
            "ul" | "ol" => {
                self.flush_loose();
                return self.list(element, list_depth);
            },
            "hr" => return self.push(BlockKind::Divider, InlineBuf::default()),
            "img" => {
                let attr = |key| element.value().attr(key).unwrap_or_default().to_owned();
                let kind = BlockKind::Image {
                    blob: attr("src"),
                    alt: attr("alt"),
                };
                return self.push(kind, InlineBuf::default());
            },
            "table" => return self.table(element),
            "div" | "section" | "article" | "main" | "header" | "footer" if is_plain(element) => {
                self.flush_loose();
                return self.children(element, list_depth);
            },
            _ if is_inline(name) => {
                inline(&mut self.loose, *element);
                self.loose_any = true;
                return;
            },
            _ => BlockKind::Raw {
                source: element.html(),
            },
        };
        let mut buf = InlineBuf::default();
        if !matches!(kind, BlockKind::Raw { .. }) {
            inline(&mut buf, *element);
        }
        self.push(kind, buf);
    }

    fn list(&mut self, list: ElementRef<'_>, depth: u8) {
        let ordered = list.value().name() == "ol";
        for child in list.child_elements() {
            match child.value().name() {
                "li" => self.list_item(child, depth, ordered),
                "ul" | "ol" => self.list(child, depth.saturating_add(1)),
                _ => {},
            }
        }
    }

    fn list_item(&mut self, item: ElementRef<'_>, depth: u8, ordered: bool) {
        let mut buf = InlineBuf::default();
        for child in item.children() {
            let nested =
                ElementRef::wrap(child).filter(|e| matches!(e.value().name(), "ul" | "ol"));
            if nested.is_none() {
                inline(&mut buf, child);
            }
        }
        self.push(
            BlockKind::ListItem {
                level: depth,
                ordered,
            },
            buf,
        );
        for nested in item
            .child_elements()
            .filter(|e| matches!(e.value().name(), "ul" | "ol"))
        {
            self.list(nested, depth.saturating_add(1));
        }
    }

    fn table(&mut self, table: ElementRef<'_>) {
        let mut rows = Vec::new();
        for row in table
            .descendants()
            .filter_map(ElementRef::wrap)
            .filter(|e| e.value().name() == "tr")
        {
            let cells = row
                .child_elements()
                .filter(|e| matches!(e.value().name(), "td" | "th"))
                .map(|cell| {
                    let mut buf = InlineBuf::default();
                    inline(&mut buf, *cell);
                    let (text, marks) = buf.take();
                    let mut block =
                        Block::new(self.ids.next_id(), BlockKind::Paragraph, text.trim());
                    block.marks = marks;
                    RichText {
                        blocks: vec![block],
                    }
                })
                .collect();
            rows.push(cells);
        }
        self.push(BlockKind::Table { rows }, InlineBuf::default());
    }

    fn push(&mut self, kind: BlockKind, mut buf: InlineBuf) {
        self.flush_loose();
        let (text, marks) = buf.take();
        let mut block = Block::new(self.ids.next_id(), kind, text);
        block.marks = marks;
        self.blocks.push(block);
    }

    fn flush_loose(&mut self) {
        if std::mem::take(&mut self.loose_any) {
            let (text, marks) = self.loose.take();
            let mut block = Block::new(self.ids.next_id(), BlockKind::Paragraph, text.trim());
            block.marks = if text.trim() == text {
                marks
            } else {
                Vec::new()
            };
            self.blocks.push(block);
        }
    }
}

fn is_inline(name: &str) -> bool {
    matches!(
        name,
        "a" | "b"
            | "strong"
            | "i"
            | "em"
            | "u"
            | "s"
            | "del"
            | "strike"
            | "code"
            | "span"
            | "mark"
            | "br"
            | "small"
            | "sub"
            | "sup"
    )
}

fn is_plain(element: ElementRef<'_>) -> bool {
    element.value().attrs().next().is_none()
}

fn inline(buf: &mut InlineBuf, node: NodeRef<'_, Node>) {
    match node.value() {
        Node::Text(text) => buf.push(text),
        Node::Element(element) => {
            let name = element.name();
            if name == "br" {
                buf.push("\n");
                return;
            }
            let mark = match name {
                "strong" | "b" => Some(MarkKind::Bold),
                "em" | "i" => Some(MarkKind::Italic),
                "u" => Some(MarkKind::Underline),
                "s" | "del" | "strike" => Some(MarkKind::Strike),
                "code" => Some(MarkKind::Code),
                "a" => element.attr("href").map(|href| MarkKind::Link {
                    href: href.to_owned(),
                }),
                "mark" => Some(MarkKind::Highlight {
                    value: style_value(element.attr("style"), "background")
                        .unwrap_or_else(|| "yellow".to_owned()),
                }),
                "span" => element
                    .attr("data-comment")
                    .map(|thread| MarkKind::Comment {
                        thread: thread.to_owned(),
                    })
                    .or_else(|| {
                        style_value(element.attr("style"), "color")
                            .map(|value| MarkKind::Color { value })
                    }),
                _ => None,
            };
            let key = mark.as_ref().map(MarkKind::key);
            if let Some(mark) = mark {
                buf.open(mark);
            }
            for child in node.children() {
                inline(buf, child);
            }
            if let Some(key) = key {
                buf.close_key(key);
            }
        },
        _ => {},
    }
}

fn style_value(style: Option<&str>, property: &str) -> Option<String> {
    style?.split(';').find_map(|decl| {
        let (name, value) = decl.split_once(':')?;
        (name.trim() == property).then(|| value.trim().to_owned())
    })
}
