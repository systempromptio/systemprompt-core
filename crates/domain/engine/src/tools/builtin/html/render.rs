//! The renderer behind the HTML converter: block and inline structure turned
//! into Markdown or plain text, whitespace collapsed outside preformatted
//! blocks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::entities::decode_entities;
use super::{Flavor, Tag};

const BLOCKS: [&str; 18] = [
    "p", "div", "section", "article", "header", "footer", "main", "nav", "aside", "table", "form",
    "figure", "dl", "dd", "dt", "address", "details", "summary",
];

#[derive(Debug)]
pub(super) struct Renderer {
    flavor: Flavor,
    out: String,
    pre: usize,
    lists: Vec<Option<usize>>,
    links: Vec<Option<String>>,
    pending_space: bool,
}

impl Renderer {
    pub(super) const fn new(flavor: Flavor) -> Self {
        Self {
            flavor,
            out: String::new(),
            pre: 0,
            lists: Vec::new(),
            links: Vec::new(),
            pending_space: false,
        }
    }

    fn markdown(&self) -> bool {
        self.flavor == Flavor::Markdown
    }

    fn at_line_start(&self) -> bool {
        self.out.is_empty() || self.out.ends_with('\n')
    }

    fn flush_space(&mut self) {
        if self.pending_space && !self.at_line_start() && !self.out.ends_with(' ') {
            self.out.push(' ');
        }
        self.pending_space = false;
    }

    fn trim_trailing(&mut self) {
        let kept = self.out.trim_end_matches([' ', '\t']).len();
        self.out.truncate(kept);
    }

    fn newline(&mut self) {
        self.trim_trailing();
        if !self.at_line_start() {
            self.out.push('\n');
        }
        self.pending_space = false;
    }

    fn block(&mut self) {
        self.newline();
        if !self.out.is_empty() && !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
    }

    pub(super) fn text(&mut self, raw: &str) {
        let decoded = decode_entities(raw);
        if self.pre > 0 {
            self.out.push_str(&decoded);
            return;
        }
        for c in decoded.chars() {
            if c.is_whitespace() {
                self.pending_space = true;
            } else {
                self.flush_space();
                self.out.push(c);
            }
        }
    }

    fn inline_marker(&mut self, tag: &Tag, marker: &str) {
        if !self.markdown() {
            return;
        }
        if !tag.closing {
            self.flush_space();
        }
        self.out.push_str(marker);
    }

    pub(super) fn tag(&mut self, tag: &Tag) {
        let name = tag.name.as_str();
        match name {
            _ if BLOCKS.contains(&name) => self.block(),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => self.heading(tag),
            "br" => {
                self.trim_trailing();
                self.out.push('\n');
                self.pending_space = false;
            },
            "hr" => {
                self.block();
                if self.markdown() {
                    self.out.push_str("---");
                }
                self.block();
            },
            "ul" | "ol" => self.list(tag),
            "li" if !tag.closing => self.list_item(),
            "li" | "tr" => self.newline(),
            "td" | "th" if tag.closing => {
                self.trim_trailing();
                self.out.push_str(" | ");
            },
            "blockquote" => {
                self.block();
                if !tag.closing && self.markdown() {
                    self.out.push_str("> ");
                }
            },
            "pre" => self.preformatted(tag),
            "code" if self.pre == 0 => self.inline_marker(tag, "`"),
            "strong" | "b" => self.inline_marker(tag, "**"),
            "em" | "i" => self.inline_marker(tag, "_"),
            "a" => self.anchor(tag),
            "img" => self.image(tag),
            _ => {},
        }
    }

    fn heading(&mut self, tag: &Tag) {
        self.block();
        if !tag.closing && self.markdown() {
            let level = tag.name[1..].parse::<usize>().unwrap_or(1);
            self.out.push_str(&"#".repeat(level));
            self.out.push(' ');
        }
    }

    fn list(&mut self, tag: &Tag) {
        if tag.closing {
            self.lists.pop();
            if self.lists.is_empty() {
                self.block();
            } else {
                self.newline();
            }
            return;
        }
        self.newline();
        let start = tag
            .attr("start")
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(1);
        self.lists.push((tag.name == "ol").then_some(start));
    }

    fn list_item(&mut self) {
        self.newline();
        let depth = self.lists.len().saturating_sub(1);
        let marker = match self.lists.last_mut() {
            Some(Some(counter)) => {
                let marker = format!("{counter}. ");
                *counter += 1;
                marker
            },
            _ => "- ".to_owned(),
        };
        self.out.push_str(&"  ".repeat(depth));
        self.out.push_str(&marker);
    }

    fn preformatted(&mut self, tag: &Tag) {
        if !tag.closing {
            self.block();
            if self.markdown() {
                self.out.push_str("```\n");
            }
            self.pre += 1;
            return;
        }
        self.pre = self.pre.saturating_sub(1);
        if !self.at_line_start() {
            self.out.push('\n');
        }
        if self.markdown() {
            self.out.push_str("```");
        }
        self.block();
    }

    fn anchor(&mut self, tag: &Tag) {
        if tag.closing {
            if let Some(Some(href)) = self.links.pop() {
                self.trim_trailing();
                self.out.push_str(&format!("]({href})"));
            }
            return;
        }
        let href = tag
            .attr("href")
            .map(str::trim)
            .filter(|href| {
                !href.is_empty() && !href.starts_with('#') && !href.starts_with("javascript:")
            })
            .map(str::to_owned);
        if self.markdown() && href.is_some() {
            self.flush_space();
            self.out.push('[');
        }
        self.links.push(if self.markdown() { href } else { None });
    }

    fn image(&mut self, tag: &Tag) {
        let alt = tag.attr("alt").unwrap_or("").trim().to_owned();
        let src = tag.attr("src").unwrap_or("").trim().to_owned();
        if self.markdown() && !src.is_empty() && !src.starts_with("data:") {
            self.flush_space();
            self.out.push_str(&format!("![{alt}]({src})"));
        } else if !alt.is_empty() {
            self.text(&alt);
        }
    }

    pub(super) fn finish(self) -> String {
        let mut result = String::with_capacity(self.out.len());
        let mut blanks = 0;
        for line in self.out.lines() {
            let line = line.trim_end();
            if line.is_empty() {
                blanks += 1;
                if blanks > 1 {
                    continue;
                }
            } else {
                blanks = 0;
            }
            result.push_str(line);
            result.push('\n');
        }
        result.trim().to_owned()
    }
}
