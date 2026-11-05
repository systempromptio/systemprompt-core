//! Places one block of each kind: heading, paragraph, list item, image,
//! divider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    Align, BAR, BODY_LEADING, BODY_SIZE, BULLET, Backdrop, CALLOUT_TINT, CODE_LEADING, CODE_SIZE,
    CODE_TINT, CONTENT_WIDTH, DIVIDER, Flow, Font, INK, INSET, LIST_INDENT, Layout, MARGIN,
    Measure, PARAGRAPH_GAP, PageCapExceeded, Placed, Rect, Rule, Run, Style, Text, aligned_x,
    fitted, lines_f32, wrap, wrap_preformatted,
};

impl Layout<'_> {
    pub(super) fn heading(
        &mut self,
        level: u8,
        runs: &[Run],
        align: Align,
    ) -> Result<(), PageCapExceeded> {
        let size = match level {
            1 => 20.0,
            2 => 16.0,
            3 => 14.0,
            _ => 12.0,
        };
        let gap_before = if self.page_is_blank() { 0.0 } else { 10.0 };
        let leading = size * 1.3;
        self.ensure(gap_before + leading)?;
        self.advance(gap_before);
        let measure = Measure {
            size,
            width: CONTENT_WIDTH,
            force_bold: true,
        };
        let lines = wrap(runs, measure);
        self.flow(
            &lines,
            Flow {
                measure,
                leading,
                x: MARGIN,
                align,
                backdrop: Backdrop::default(),
            },
        )?;
        self.advance(4.0);
        Ok(())
    }

    pub(super) fn paragraph(
        &mut self,
        style: Style,
        runs: &[Run],
        align: Align,
    ) -> Result<(), PageCapExceeded> {
        let (size, leading, inset, backdrop) = match style {
            Style::Body => (BODY_SIZE, BODY_LEADING, 0.0, Backdrop::default()),
            Style::Quote => (
                BODY_SIZE,
                BODY_LEADING,
                INSET,
                Backdrop {
                    fill: None,
                    bar: Some(BAR),
                },
            ),
            Style::Callout => (
                BODY_SIZE,
                BODY_LEADING,
                INSET,
                Backdrop {
                    fill: Some(CALLOUT_TINT),
                    bar: Some(BAR),
                },
            ),
            Style::Code => (
                CODE_SIZE,
                CODE_LEADING,
                4.0,
                Backdrop {
                    fill: Some(CODE_TINT),
                    bar: None,
                },
            ),
            Style::Lines => (CODE_SIZE, CODE_LEADING, 0.0, Backdrop::default()),
        };
        let measure = Measure {
            size,
            width: inset.mul_add(-2.0, CONTENT_WIDTH),
            force_bold: false,
        };
        let lines = if matches!(style, Style::Code | Style::Lines) {
            wrap_preformatted(runs, measure)
        } else {
            wrap(runs, measure)
        };
        self.flow(
            &lines,
            Flow {
                measure,
                leading,
                x: MARGIN + inset,
                align,
                backdrop,
            },
        )?;
        if style != Style::Lines {
            self.advance(PARAGRAPH_GAP);
        }
        Ok(())
    }

    pub(super) fn list_item(
        &mut self,
        depth: usize,
        marker: &str,
        runs: &[Run],
    ) -> Result<(), PageCapExceeded> {
        let indent = LIST_INDENT * lines_f32(depth + 1);
        let measure = Measure {
            size: BODY_SIZE,
            width: CONTENT_WIDTH - indent,
            force_bold: false,
        };
        self.ensure(BODY_LEADING)?;
        let y = self.y - BODY_SIZE;
        let marker_width = Font::Sans.text_width(marker, BODY_SIZE);
        let x = if marker == BULLET {
            MARGIN + indent - 10.0
        } else {
            MARGIN + indent - marker_width - 3.0
        };
        self.page_mut().texts.push(Text {
            x,
            y,
            font: Font::Sans,
            size: BODY_SIZE,
            color: INK,
            content: marker.to_owned(),
        });
        let lines = wrap(runs, measure);
        self.flow(
            &lines,
            Flow {
                measure,
                leading: BODY_LEADING,
                x: MARGIN + indent,
                align: Align::Left,
                backdrop: Backdrop::default(),
            },
        )?;
        self.advance(2.0);
        Ok(())
    }

    pub(super) fn image(
        &mut self,
        picture: usize,
        (width, height): (u32, u32),
        align: Align,
    ) -> Result<(), PageCapExceeded> {
        let Some(held) = self.pictures.get(picture) else {
            return Ok(());
        };
        let (px_w, px_h) = if width > 0 && height > 0 {
            (width, height)
        } else {
            (held.width, held.height)
        };
        let (w, h) = fitted(px_w, px_h);
        self.ensure(h)?;
        let x = aligned_x(MARGIN, CONTENT_WIDTH, w, align);
        let y = self.y - h;
        self.page_mut().pictures.push(Placed {
            picture,
            rect: Rect { x, y, w, h },
        });
        self.advance(h + PARAGRAPH_GAP);
        Ok(())
    }

    pub(super) fn divider(&mut self) -> Result<(), PageCapExceeded> {
        self.ensure(BODY_LEADING)?;
        self.advance(BODY_LEADING / 2.0);
        let y = self.y;
        self.page_mut().rules.push(Rule {
            x1: MARGIN,
            y1: y,
            x2: MARGIN + CONTENT_WIDTH,
            y2: y,
            color: DIVIDER,
            width: 0.75,
        });
        self.advance(BODY_LEADING / 2.0);
        Ok(())
    }
}
