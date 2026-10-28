//! Flows blocks onto A4 pages. The page count is bounded while laying out,
//! so an oversized document stops at the cap instead of being paginated in
//! full and then refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::blocks::{Block, Style};
use super::fonts::Font;
use super::wrap::wrap;

pub(super) const PAGE_WIDTH: f32 = 595.276;
pub(super) const PAGE_HEIGHT: f32 = 841.89;
pub(super) const MARGIN: f32 = 56.0;
pub(super) const CONTENT_WIDTH: f32 = PAGE_WIDTH - 2.0 * MARGIN;
pub(super) const BODY_SIZE: f32 = 11.0;
pub(super) const BODY_LEADING: f32 = 14.0;
pub(super) const MONO_SIZE: f32 = 9.5;
pub(super) const MONO_LEADING: f32 = 12.0;
pub(super) const FOOTER_SIZE: f32 = 9.0;
const PARAGRAPH_GAP: f32 = 6.0;
const LIST_INDENT: f32 = 16.0;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Text {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) font: Font,
    pub(super) size: f32,
    pub(super) content: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Face {
    pub(super) font: Font,
    pub(super) size: f32,
    pub(super) leading: f32,
}

impl Face {
    pub(super) const BODY: Self = Self {
        font: Font::Sans,
        size: BODY_SIZE,
        leading: BODY_LEADING,
    };
    pub(super) const ITALIC: Self = Self {
        font: Font::SansItalic,
        size: BODY_SIZE,
        leading: BODY_LEADING,
    };
    pub(super) const MONO: Self = Self {
        font: Font::Mono,
        size: MONO_SIZE,
        leading: MONO_LEADING,
    };

    const fn heading(level: u8) -> Self {
        let size = match level {
            1 => 20.0,
            2 => 16.0,
            3 => 14.0,
            _ => 12.0,
        };
        Self {
            font: Font::SansBold,
            size,
            leading: size * 1.3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Rule {
    pub(super) x1: f32,
    pub(super) y1: f32,
    pub(super) x2: f32,
    pub(super) y2: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Page {
    pub(super) texts: Vec<Text>,
    pub(super) rules: Vec<Rule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the rendered PDF exceeds {max_pages} pages")]
pub struct PageCapExceeded {
    pub max_pages: usize,
}

#[derive(Debug)]
pub(super) struct Layout {
    pages: Vec<Page>,
    y: f32,
    max_pages: usize,
}

impl Layout {
    pub(super) fn paginate(
        blocks: &[Block],
        max_pages: usize,
    ) -> Result<Vec<Page>, PageCapExceeded> {
        let mut layout = Self {
            pages: vec![Page::default()],
            y: PAGE_HEIGHT - MARGIN,
            max_pages,
        };
        for block in blocks {
            layout.place(block)?;
        }
        Ok(layout.pages)
    }

    pub(super) fn page_is_blank(&self) -> bool {
        self.pages.last().is_some_and(|page| page.texts.is_empty())
    }

    pub(super) fn new_page(&mut self) -> Result<(), PageCapExceeded> {
        if self.pages.len() >= self.max_pages {
            return Err(PageCapExceeded {
                max_pages: self.max_pages,
            });
        }
        self.pages.push(Page::default());
        self.y = PAGE_HEIGHT - MARGIN;
        Ok(())
    }

    pub(super) fn ensure(&mut self, height: f32) -> Result<(), PageCapExceeded> {
        if self.y - height < MARGIN && !self.page_is_blank() {
            self.new_page()?;
        }
        Ok(())
    }

    pub(super) const fn available(&self) -> f32 {
        self.y - MARGIN
    }

    pub(super) const fn y(&self) -> f32 {
        self.y
    }

    pub(super) fn advance(&mut self, height: f32) {
        self.y -= height;
    }

    pub(super) fn page_mut(&mut self) -> &mut Page {
        if self.pages.is_empty() {
            self.pages.push(Page::default());
        }
        let last = self.pages.len() - 1;
        &mut self.pages[last]
    }

    fn place(&mut self, block: &Block) -> Result<(), PageCapExceeded> {
        match block {
            Block::Heading { level, text } => self.heading(*level, text),
            Block::Paragraph { style, text } => self.paragraph(*style, text),
            Block::ListItem { depth, text } => self.list_item(*depth, text),
            Block::Table { rows } => super::table::place(self, rows),
            Block::PageBreak => {
                if self.page_is_blank() {
                    Ok(())
                } else {
                    self.new_page()
                }
            },
        }
    }

    fn heading(&mut self, level: u8, text: &str) -> Result<(), PageCapExceeded> {
        let face = Face::heading(level);
        let gap_before = if self.page_is_blank() { 0.0 } else { 10.0 };
        let lines = wrap(text, face.font, face.size, CONTENT_WIDTH);
        self.ensure(gap_before + face.leading)?;
        self.advance(gap_before);
        self.lines(&lines, face, MARGIN)?;
        self.advance(4.0);
        Ok(())
    }

    fn paragraph(&mut self, style: Style, text: &str) -> Result<(), PageCapExceeded> {
        let face = match style {
            Style::Body => Face::BODY,
            Style::Italic => Face::ITALIC,
            Style::Mono => Face::MONO,
        };
        let lines = wrap(text, face.font, face.size, CONTENT_WIDTH);
        self.lines(&lines, face, MARGIN)?;
        if style != Style::Mono {
            self.advance(PARAGRAPH_GAP);
        }
        Ok(())
    }

    fn list_item(&mut self, depth: usize, text: &str) -> Result<(), PageCapExceeded> {
        let indent = LIST_INDENT * lines_f32(depth + 1);
        let lines = wrap(text, Font::Sans, BODY_SIZE, CONTENT_WIDTH - indent);
        self.ensure(BODY_LEADING)?;
        let y = self.y - BODY_SIZE;
        self.page_mut().texts.push(Text {
            x: MARGIN + indent - 10.0,
            y,
            font: Font::Sans,
            size: BODY_SIZE,
            content: "\u{2022}".to_owned(),
        });
        self.lines(&lines, Face::BODY, MARGIN + indent)?;
        self.advance(2.0);
        Ok(())
    }

    pub(super) fn lines(
        &mut self,
        lines: &[String],
        face: Face,
        x: f32,
    ) -> Result<(), PageCapExceeded> {
        for line in lines {
            self.ensure(face.leading)?;
            let y = self.y - face.size;
            self.page_mut().texts.push(Text {
                x,
                y,
                font: face.font,
                size: face.size,
                content: line.clone(),
            });
            self.advance(face.leading);
        }
        Ok(())
    }
}

#[must_use]
pub(super) fn lines_f32(count: usize) -> f32 {
    f32::from(u16::try_from(count).unwrap_or(u16::MAX))
}

#[must_use]
pub(super) fn whole(value: f32) -> usize {
    usize::from(value.max(0.0).floor().min(f32::from(u16::MAX)) as u16)
}

pub(super) fn footer(pages: &mut [Page], name: &str) {
    let total = pages.len();
    for (index, page) in pages.iter_mut().enumerate() {
        let label = format!("Page {} of {total}", index + 1);
        let width = Font::Sans.text_width(&label, FOOTER_SIZE);
        let mut shown: String = name.split_whitespace().collect::<Vec<_>>().join(" ");
        let room = CONTENT_WIDTH - width - 20.0;
        loop {
            let fits = Font::SansItalic.text_width(&shown, FOOTER_SIZE) <= room;
            if fits || shown.pop().is_none() {
                break;
            }
        }
        page.texts.push(Text {
            x: MARGIN,
            y: MARGIN / 2.0,
            font: Font::SansItalic,
            size: FOOTER_SIZE,
            content: shown,
        });
        page.texts.push(Text {
            x: PAGE_WIDTH - MARGIN - width,
            y: MARGIN / 2.0,
            font: Font::Sans,
            size: FOOTER_SIZE,
            content: label,
        });
    }
}
