//! Greedy line breaking over styled runs, measured with the bundled fonts'
//! advance widths. A word wider than the line breaks at the character that
//! overflows. A word may span runs, so `bo**ld**` never splits at the mark.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::fonts::Font;
use super::runs::Run;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Measure {
    pub(super) size: f32,
    pub(super) width: f32,
    pub(super) force_bold: bool,
}

impl Measure {
    #[must_use]
    pub(super) const fn font_of(&self, run: &Run) -> Font {
        Font::styled(run.bold || self.force_bold, run.italic, run.mono)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Line {
    pub(super) pieces: Vec<Run>,
    pub(super) width: f32,
}

#[derive(Default)]
struct Builder<'a> {
    runs: &'a [Run],
    lines: Vec<Line>,
    chars: Vec<(char, usize)>,
    width: f32,
}

impl Builder<'_> {
    fn flush(&mut self) {
        let mut line = Line {
            pieces: Vec::new(),
            width: self.width,
        };
        for (c, at) in self.chars.drain(..) {
            match line.pieces.last_mut() {
                Some(piece) if piece.same_style(&self.runs[at]) => piece.text.push(c),
                _ => line.pieces.push(Run {
                    text: c.to_string(),
                    ..self.runs[at].clone()
                }),
            }
        }
        self.width = 0.0;
        self.lines.push(line);
    }

    fn push(&mut self, c: char, at: usize, width: f32) {
        self.chars.push((c, at));
        self.width += width;
    }
}

fn flatten(runs: &[Run]) -> impl Iterator<Item = (char, usize)> + '_ {
    runs.iter()
        .enumerate()
        .flat_map(|(at, run)| run.text.chars().map(move |c| (c, at)))
}

#[must_use]
pub(super) fn wrap(runs: &[Run], measure: Measure) -> Vec<Line> {
    let width_of = |c: char, at: usize| measure.font_of(&runs[at]).char_width(c, measure.size);
    let mut out = Builder {
        runs,
        ..Builder::default()
    };
    let mut word: Vec<(char, usize)> = Vec::new();
    let place = |word: &mut Vec<(char, usize)>, out: &mut Builder<'_>| {
        if word.is_empty() {
            return;
        }
        let total: f32 = word.iter().map(|(c, at)| width_of(*c, *at)).sum();
        let space = width_of(' ', word[0].1);
        if !out.chars.is_empty() && out.width + space + total > measure.width {
            out.flush();
        }
        if !out.chars.is_empty() {
            out.push(' ', word[0].1, space);
        }
        for (c, at) in word.drain(..) {
            let advance = width_of(c, at);
            if !out.chars.is_empty() && out.width + advance > measure.width {
                out.flush();
            }
            out.push(c, at, advance);
        }
    };
    for (c, at) in flatten(runs) {
        if c.is_whitespace() {
            place(&mut word, &mut out);
        } else {
            word.push((c, at));
        }
    }
    place(&mut word, &mut out);
    if !out.chars.is_empty() || out.lines.is_empty() {
        out.flush();
    }
    out.lines
}

/// Keeps every space and line break, for code and plain text.
#[must_use]
pub(super) fn wrap_preformatted(runs: &[Run], measure: Measure) -> Vec<Line> {
    let mut out = Builder {
        runs,
        ..Builder::default()
    };
    let mut ended_on_break = true;
    for (c, at) in flatten(runs) {
        if c == '\n' {
            out.flush();
            ended_on_break = true;
            continue;
        }
        ended_on_break = false;
        let c = if c == '\t' { ' ' } else { c };
        let advance = measure.font_of(&runs[at]).char_width(c, measure.size);
        if !out.chars.is_empty() && out.width + advance > measure.width {
            out.flush();
        }
        out.push(c, at, advance);
    }
    if !ended_on_break || out.lines.is_empty() {
        out.flush();
    }
    out.lines
}
