//! Flows blocks onto A4 pages. The page count is bounded while laying out,
//! so an oversized document stops at the cap instead of being paginated in
//! full and then refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::blocks::{BULLET, Block, Style};
use super::fonts::Font;
use super::images::Picture;
use super::layout_model::{
    Align, CONTENT_WIDTH, Fill, LinkArea, MARGIN, PAGE_HEIGHT, PAGE_WIDTH, Page, PageCapExceeded,
    Placed, Rect, Rule, Text, lines_f32,
};
use super::runs::{Rgb, Run};
use super::wrap::{Line, Measure, wrap, wrap_preformatted};

mod placing;

const BODY_SIZE: f32 = 11.0;
const BODY_LEADING: f32 = 14.5;
const CODE_SIZE: f32 = 9.5;
const CODE_LEADING: f32 = 12.5;
const FOOTER_SIZE: f32 = 9.0;
const PARAGRAPH_GAP: f32 = 6.0;
const LIST_INDENT: f32 = 16.0;
const INSET: f32 = 12.0;
const INK: Rgb = [0x1f, 0x23, 0x28];
const MUTED: Rgb = [0x5a, 0x62, 0x6c];
const CODE_TINT: Rgb = [0xf3, 0xf4, 0xf6];
const CALLOUT_TINT: Rgb = [0xea, 0xf1, 0xfb];
const BAR: Rgb = [0xb8, 0xc0, 0xcc];
const DIVIDER: Rgb = [0xc8, 0xcd, 0xd4];
const PX_TO_PT: f32 = 0.75;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Backdrop {
    fill: Option<Rgb>,
    bar: Option<Rgb>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Flow {
    pub(super) measure: Measure,
    pub(super) leading: f32,
    pub(super) x: f32,
    pub(super) align: Align,
    pub(super) backdrop: Backdrop,
}

#[derive(Debug)]
pub(super) struct Layout<'a> {
    pages: Vec<Page>,
    y: f32,
    max_pages: usize,
    pictures: &'a [Picture],
}

impl<'a> Layout<'a> {
    pub(super) fn paginate(
        blocks: &[Block],
        pictures: &'a [Picture],
        max_pages: usize,
    ) -> Result<Vec<Page>, PageCapExceeded> {
        let mut layout = Self {
            pages: vec![Page::default()],
            y: PAGE_HEIGHT - MARGIN,
            max_pages,
            pictures,
        };
        for block in blocks {
            layout.place(block)?;
        }
        Ok(layout.pages)
    }

    pub(super) fn page_is_blank(&self) -> bool {
        self.pages.last().is_some_and(Page::is_blank)
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
            Block::Heading { level, runs, align } => self.heading(*level, runs, *align),
            Block::Paragraph { style, runs, align } => self.paragraph(*style, runs, *align),
            Block::ListItem {
                depth,
                marker,
                runs,
            } => self.list_item(*depth, marker, runs),
            Block::Table { rows, header_rows } => super::table::place(self, rows, *header_rows),
            Block::Image {
                picture,
                width,
                height,
                align,
            } => self.image(*picture, (*width, *height), *align),
            Block::Divider => self.divider(),
            Block::PageBreak => {
                if self.page_is_blank() {
                    Ok(())
                } else {
                    self.new_page()
                }
            },
        }
    }

    pub(super) fn flow(&mut self, lines: &[Line], flow: Flow) -> Result<(), PageCapExceeded> {
        for line in lines {
            self.ensure(flow.leading)?;
            let top = self.y;
            self.backdrop(top, flow);
            let baseline = top - flow.measure.size;
            let x = aligned_x(flow.x, flow.measure.width, line.width, flow.align);
            self.draw_line(line, x, baseline, flow.measure);
            self.advance(flow.leading);
        }
        Ok(())
    }

    fn backdrop(&mut self, top: f32, flow: Flow) {
        let bottom = top - flow.leading;
        let page = self.page_mut();
        if let Some(color) = flow.backdrop.fill {
            page.fills.push(Fill {
                rect: Rect {
                    x: MARGIN,
                    y: bottom,
                    w: CONTENT_WIDTH,
                    h: flow.leading,
                },
                color,
            });
        }
        if let Some(color) = flow.backdrop.bar {
            page.fills.push(Fill {
                rect: Rect {
                    x: MARGIN,
                    y: bottom,
                    w: 2.5,
                    h: flow.leading,
                },
                color,
            });
        }
    }

    pub(super) fn draw_line(&mut self, line: &Line, x: f32, baseline: f32, measure: Measure) {
        let page = self.page_mut();
        let size = measure.size;
        let mut at = x;
        for piece in &line.pieces {
            let font = measure.font_of(piece);
            let width = font.text_width(&piece.text, size);
            let color = piece.color.unwrap_or(INK);
            if let Some(highlight) = piece.highlight {
                page.fills.push(Fill {
                    rect: Rect {
                        x: at,
                        y: size.mul_add(-0.25, baseline),
                        w: width,
                        h: size * 1.2,
                    },
                    color: highlight,
                });
            }
            let mut decorate = |offset: f32| {
                page.rules.push(Rule {
                    x1: at,
                    y1: baseline + offset,
                    x2: at + width,
                    y2: baseline + offset,
                    color,
                    width: size * 0.06,
                });
            };
            if piece.underline {
                decorate(-size * 0.12);
            }
            if piece.strike {
                decorate(size * 0.3);
            }
            if let Some(href) = &piece.link {
                page.links.push(LinkArea {
                    rect: Rect {
                        x: at,
                        y: size.mul_add(-0.25, baseline),
                        w: width,
                        h: size * 1.2,
                    },
                    href: href.clone(),
                });
            }
            page.texts.push(Text {
                x: at,
                y: baseline,
                font,
                size,
                color,
                content: piece.text.clone(),
            });
            at += width;
        }
    }
}

fn fitted(px_w: u32, px_h: u32) -> (f32, f32) {
    let as_points = |px: u32| f32::from(u16::try_from(px).unwrap_or(u16::MAX)) * PX_TO_PT;
    let (w, h) = (as_points(px_w).max(1.0), as_points(px_h).max(1.0));
    let room_h = 2.0f32.mul_add(-MARGIN, PAGE_HEIGHT);
    let scale = (CONTENT_WIDTH / w).min(room_h / h).min(1.0);
    (w * scale, h * scale)
}

fn aligned_x(x: f32, room: f32, used: f32, align: Align) -> f32 {
    match align {
        Align::Left => x,
        Align::Center => x + (room - used).max(0.0) / 2.0,
        Align::Right => x + (room - used).max(0.0),
    }
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
            color: MUTED,
            content: shown,
        });
        page.texts.push(Text {
            x: PAGE_WIDTH - MARGIN - width,
            y: MARGIN / 2.0,
            font: Font::Sans,
            size: FOOTER_SIZE,
            color: MUTED,
            content: label,
        });
    }
}
