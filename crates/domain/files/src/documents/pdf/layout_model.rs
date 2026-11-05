//! What a laid-out page holds, and the page geometry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::fonts::Font;
use super::runs::Rgb;

pub(super) const PAGE_WIDTH: f32 = 595.276;
pub(super) const PAGE_HEIGHT: f32 = 841.89;
pub(super) const MARGIN: f32 = 56.0;
pub(super) const CONTENT_WIDTH: f32 = PAGE_WIDTH - 2.0 * MARGIN;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Align {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Text {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) font: Font,
    pub(super) size: f32,
    pub(super) color: Rgb,
    pub(super) content: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Rule {
    pub(super) x1: f32,
    pub(super) y1: f32,
    pub(super) x2: f32,
    pub(super) y2: f32,
    pub(super) color: Rgb,
    pub(super) width: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Rect {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) w: f32,
    pub(super) h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Fill {
    pub(super) rect: Rect,
    pub(super) color: Rgb,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Placed {
    pub(super) picture: usize,
    pub(super) rect: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct LinkArea {
    pub(super) rect: Rect,
    pub(super) href: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Page {
    pub(super) fills: Vec<Fill>,
    pub(super) pictures: Vec<Placed>,
    pub(super) rules: Vec<Rule>,
    pub(super) texts: Vec<Text>,
    pub(super) links: Vec<LinkArea>,
}

impl Page {
    #[must_use]
    pub(super) const fn is_blank(&self) -> bool {
        self.texts.is_empty() && self.pictures.is_empty() && self.rules.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the rendered PDF exceeds {max_pages} pages")]
pub struct PageCapExceeded {
    pub max_pages: usize,
}

#[must_use]
pub(super) fn lines_f32(count: usize) -> f32 {
    f32::from(u16::try_from(count).unwrap_or(u16::MAX))
}

#[must_use]
pub(super) fn whole(value: f32) -> usize {
    usize::from(value.max(0.0).floor().min(f32::from(u16::MAX)) as u16)
}
