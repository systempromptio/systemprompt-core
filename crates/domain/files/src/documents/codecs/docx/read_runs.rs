//! The runs of one paragraph read as text and marks.
//!
//! Runs inside hyperlinks, insertions, smart tags, content controls and
//! simple fields are read through their container; a complex field's result
//! runs carry a `field` mark when its code is one the model knows, and a
//! `TOC` field makes the paragraph a table of contents. Drawings are
//! collected for the caller; VML pictures, embedded objects and markup
//! compatibility blocks flag the paragraph as one the model cannot hold.
//! The colour and underline Word gives a hyperlink are its look, not marks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::codecs::xml_tree::Element;
use crate::documents::model::{FIELD_CODES, Mark, MarkKind};

use super::read::{ReadCtx, is_on};

const CONTAINERS: [&str; 8] = [
    "w:ins",
    "w:smartTag",
    "w:customXml",
    "w:sdt",
    "w:sdtContent",
    "w:dir",
    "w:bdo",
    "w:moveTo",
];

const OBJECTS: [&str; 3] = ["w:pict", "w:object", "mc:AlternateContent"];

pub(super) const CODE_FONTS: [&str; 3] = ["Courier New", "Consolas", "Courier"];

pub(super) const LINK_COLOR: &str = "#0563C1";

pub(super) struct Runs<'a> {
    xml: &'a str,
    ctx: &'a ReadCtx,
    pub(super) text: String,
    pub(super) marks: Vec<Mark>,
    pub(super) drawings: Vec<&'a Element>,
    pub(super) object: bool,
    pub(super) page_break: bool,
    pub(super) toc: Option<u8>,
    link: Option<String>,
    field: Option<String>,
    instr: Option<String>,
    len: usize,
}

impl<'a> Runs<'a> {
    pub(super) const fn new(xml: &'a str, ctx: &'a ReadCtx) -> Self {
        Self {
            xml,
            ctx,
            text: String::new(),
            marks: Vec::new(),
            drawings: Vec::new(),
            object: false,
            page_break: false,
            toc: None,
            link: None,
            field: None,
            instr: None,
            len: 0,
        }
    }

    pub(super) fn walk(&mut self, parent: &'a Element) {
        for child in &parent.children {
            match child.name.as_str() {
                "w:r" => self.run(child),
                "w:hyperlink" => {
                    let href = child
                        .attr("r:id")
                        .and_then(|id| self.ctx.rels.get(id).cloned())
                        .or_else(|| child.attr("w:anchor").map(|a| format!("#{a}")));
                    let outer = std::mem::replace(&mut self.link, href);
                    self.walk(child);
                    self.link = outer;
                },
                "w:fldSimple" => {
                    let code = child.attr("w:instr").and_then(field_code);
                    let outer = std::mem::replace(&mut self.field, code);
                    self.walk(child);
                    self.field = outer;
                },
                name if CONTAINERS.contains(&name) => self.walk(child),
                name if OBJECTS.contains(&name) => self.object = true,
                _ => {},
            }
        }
    }

    fn run(&mut self, run: &'a Element) {
        let base = run
            .child("w:rPr")
            .map(|rpr| self.run_marks(rpr))
            .unwrap_or_default();
        for child in &run.children {
            match child.name.as_str() {
                "w:t" => {
                    if let Ok(text) = child.text("word/document.xml", self.xml) {
                        self.push(&text, &base);
                    }
                },
                "w:tab" | "w:ptab" => self.push("\t", &base),
                "w:br" | "w:cr" => match child.attr("w:type") {
                    Some("page") => self.page_break = true,
                    _ => self.push("\n", &base),
                },
                "w:fldChar" => self.fld_char(child.attr("w:fldCharType").unwrap_or_default()),
                "w:instrText" => {
                    let text = child.text("word/document.xml", self.xml);
                    if let (Ok(text), Some(instr)) = (text, self.instr.as_mut()) {
                        instr.push_str(&text);
                    }
                },
                "w:drawing" => self.drawings.push(child),
                name if OBJECTS.contains(&name) => self.object = true,
                _ => {},
            }
        }
    }

    fn fld_char(&mut self, kind: &str) {
        match kind {
            "begin" => self.instr = Some(String::new()),
            "separate" => {
                let instr = self.instr.take().unwrap_or_default();
                if let Some(levels) = toc_levels(&instr) {
                    self.toc = Some(levels);
                }
                self.field = field_code(&instr);
            },
            "end" => {
                self.instr = None;
                self.field = None;
            },
            _ => {},
        }
    }

    fn push(&mut self, piece: &str, base: &[MarkKind]) {
        if self.instr.is_some() || self.toc.is_some() {
            return;
        }
        let len = piece.chars().count();
        if len == 0 {
            return;
        }
        let (start, end) = (self.len, self.len + len);
        let link = self.link.clone().map(|href| MarkKind::Link { href });
        let field = self.field.clone().map(|code| MarkKind::Field { code });
        for kind in base.iter().cloned().chain(link).chain(field) {
            extend(&mut self.marks, start, end, kind);
        }
        self.text.push_str(piece);
        self.len = end;
    }

    fn run_marks(&self, rpr: &Element) -> Vec<MarkKind> {
        let val = |name: &str| rpr.child(name).and_then(|e| e.attr("w:val"));
        let flag = |name: &str| rpr.child(name).is_some_and(is_on);
        let in_link = self.link.is_some();
        let mut marks = Vec::new();
        for (on, kind) in [
            (flag("w:b"), MarkKind::Bold),
            (flag("w:i"), MarkKind::Italic),
            (flag("w:u") && !in_link, MarkKind::Underline),
            (flag("w:strike") || flag("w:dstrike"), MarkKind::Strike),
            (flag("w:caps"), MarkKind::Caps),
        ] {
            if on {
                marks.push(kind);
            }
        }
        match rpr.child("w:rFonts").and_then(|f| f.attr("w:ascii")) {
            Some(font) if CODE_FONTS.contains(&font) => marks.push(MarkKind::Code),
            Some(font) => marks.push(MarkKind::Font {
                value: font.to_owned(),
            }),
            None => {},
        }
        if let Some(color) = val("w:color")
            .filter(|c| *c != "auto")
            .map(|c| format!("#{c}"))
            && !(in_link && color.eq_ignore_ascii_case(LINK_COLOR))
        {
            marks.push(MarkKind::Color { value: color });
        }
        if let Some(name) = val("w:highlight").filter(|h| *h != "none") {
            marks.push(MarkKind::Highlight {
                value: name.to_owned(),
            });
        } else if let Some(fill) = rpr
            .child("w:shd")
            .and_then(|s| s.attr("w:fill"))
            .filter(|f| *f != "auto")
        {
            marks.push(MarkKind::Highlight {
                value: format!("#{fill}"),
            });
        }
        if let Some(size) = val("w:sz").and_then(half_points) {
            marks.push(MarkKind::Size { value: size });
        }
        match val("w:vertAlign") {
            Some("superscript") => marks.push(MarkKind::Superscript),
            Some("subscript") => marks.push(MarkKind::Subscript),
            _ => {},
        }
        marks
    }
}

fn extend(marks: &mut Vec<Mark>, start: usize, end: usize, kind: MarkKind) {
    match marks
        .iter_mut()
        .find(|mark| mark.end == start && mark.kind == kind)
    {
        Some(mark) => mark.end = end,
        None => marks.push(Mark { start, end, kind }),
    }
}

pub(super) fn half_points(value: &str) -> Option<String> {
    let half = value.parse::<u32>().ok()?;
    Some(if half % 2 == 0 {
        (half / 2).to_string()
    } else {
        format!("{}.5", half / 2)
    })
}

fn field_code(instr: &str) -> Option<String> {
    let code = instr.split_whitespace().next()?.to_ascii_uppercase();
    FIELD_CODES.contains(&code.as_str()).then_some(code)
}

fn toc_levels(instr: &str) -> Option<u8> {
    let mut words = instr.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("TOC") {
        return None;
    }
    let range = instr.split("\\o").nth(1).unwrap_or_default();
    let last = range
        .trim()
        .trim_matches('"')
        .split('-')
        .nth(1)
        .and_then(|n| n.trim_matches('"').parse::<u8>().ok());
    Some(last.unwrap_or(3).clamp(1, 9))
}
