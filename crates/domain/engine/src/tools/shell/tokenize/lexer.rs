//! The character-level lexer behind `split_commands`: words, quotes,
//! redirections, separators and substitutions, with the per-dialect rules in a
//! sibling module.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::redirects::HereDocument;
use super::{Dialect, Segment};

const MAX_DEPTH: usize = 64;

#[derive(Debug)]
pub(super) struct Lexer {
    pub(super) chars: Vec<char>,
    pub(super) at: usize,
    pub(super) dialect: Dialect,
    pub(super) segments: Vec<Segment>,
    pub(super) nested: Vec<Segment>,
    pub(super) current: Segment,
    pub(super) word: String,
    pub(super) has_word: bool,
    pub(super) redirect_next: bool,
    pub(super) quoted_at: Option<usize>,
    pub(super) here_next: Option<bool>,
    pub(super) here_documents: Vec<HereDocument>,
    pub(super) depth: usize,
    pub(super) too_deep: bool,
}

impl Lexer {
    pub(super) fn new(command: &str, dialect: Dialect, depth: usize) -> Self {
        Self {
            chars: command.chars().collect(),
            at: 0,
            dialect,
            segments: Vec::new(),
            nested: Vec::new(),
            current: Segment::default(),
            word: String::new(),
            has_word: false,
            redirect_next: false,
            quoted_at: None,
            here_next: None,
            here_documents: Vec::new(),
            depth,
            too_deep: depth > MAX_DEPTH,
        }
    }

    pub(super) fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    pub(super) fn push(&mut self, c: char) {
        self.word.push(c);
        self.has_word = true;
    }

    pub(super) const fn mark_quoted(&mut self) {
        if self.quoted_at.is_none() {
            self.quoted_at = Some(self.word.len());
        }
    }

    pub(super) fn flush_word(&mut self) {
        if !self.has_word {
            return;
        }
        let word = std::mem::take(&mut self.word);
        let quoted_at = self.quoted_at.take();
        if let Some(strip_tabs) = self.here_next.take() {
            self.here_documents.push(HereDocument {
                delimiter: word,
                strip_tabs,
                expands: quoted_at.is_none(),
            });
        } else if self.redirect_next {
            self.current.redirects.push(word);
            self.redirect_next = false;
        } else {
            if self.opens_function_body(&word) {
                let definition = std::mem::take(&mut self.current);
                self.segments.push(definition);
            }
            if quoted_at.is_some_and(|at| word.find('=').is_some_and(|equals| at <= equals)) {
                self.current.quoted_names.push(self.current.words.len());
            }
            self.current.words.push(word);
        }
        self.has_word = false;
    }

    fn opens_function_body(&self, word: &str) -> bool {
        self.dialect == Dialect::Posix
            && word == "{"
            && matches!(self.current.words.as_slice(), [keyword, _] if keyword == "function")
    }

    pub(super) fn end_segment(&mut self) {
        self.flush_word();
        self.redirect_next = false;
        self.here_next = None;
        let segment = std::mem::take(&mut self.current);
        if !segment.words.is_empty() || !segment.redirects.is_empty() {
            self.segments.push(segment);
        }
    }

    pub(super) fn run(mut self) -> Option<Vec<Segment>> {
        while let Some(c) = self.peek(0) {
            if self.too_deep {
                return None;
            }
            match c {
                ' ' | '\t' => {
                    self.flush_word();
                    self.at += 1;
                },
                '\n' => {
                    self.end_segment();
                    self.at += 1;
                    self.here_bodies();
                },
                '\r' => {
                    self.end_segment();
                    self.at += 1;
                },
                _ => match self.dialect {
                    Dialect::Posix => self.posix(c),
                    Dialect::PowerShell => self.powershell(c),
                    Dialect::Cmd => self.cmd(c),
                },
            }
        }
        self.end_segment();
        if self.too_deep {
            return None;
        }
        let mut all = self.segments;
        all.extend(self.nested);
        Some(all)
    }

    pub(super) fn skip_comment(&mut self) {
        while let Some(c) = self.peek(0) {
            if matches!(c, '\n' | '\r') {
                break;
            }
            self.at += 1;
        }
    }

    pub(super) fn quoted_literal(&mut self, quote: char, doubled_escape: bool) {
        self.mark_quoted();
        self.at += 1;
        self.has_word = true;
        while let Some(c) = self.peek(0) {
            self.at += 1;
            if c == quote {
                if doubled_escape && self.peek(0) == Some(quote) {
                    self.at += 1;
                    self.word.push(quote);
                    continue;
                }
                return;
            }
            self.word.push(c);
        }
    }

    pub(super) fn take_group(&mut self) -> String {
        let mut depth = 0usize;
        let mut quote: Option<char> = None;
        let mut out = String::new();
        let mut previous: Option<char> = None;
        while let Some(c) = self.peek(0) {
            self.at += 1;
            if let Some(open) = quote {
                if c == open {
                    quote = None;
                }
                out.push(c);
                previous = Some(c);
                continue;
            }
            match c {
                '\'' | '"' => quote = Some(c),
                '#' if depth > 0 && self.starts_comment(previous) => {
                    self.skip_comment();
                    continue;
                },
                '$' if self.peek(0) == Some('{') => {
                    out.push(c);
                    self.take_braced(&mut out);
                    previous = Some('}');
                    continue;
                },
                '(' => {
                    depth += 1;
                    if depth == 1 {
                        continue;
                    }
                },
                ')' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                },
                _ => {},
            }
            out.push(c);
            previous = Some(c);
        }
        out
    }

    const fn starts_comment(&self, previous: Option<char>) -> bool {
        match (self.dialect, previous) {
            (Dialect::Cmd, _) => false,
            (_, None) => true,
            (_, Some(c)) if c.is_whitespace() => true,
            (Dialect::PowerShell, Some(c)) => matches!(c, '(' | ')' | '{' | '}' | ';' | '|' | '&'),
            (Dialect::Posix, Some(c)) => matches!(c, '(' | ';' | '|' | '&'),
        }
    }

    fn take_braced(&mut self, out: &mut String) {
        let mut depth = 0usize;
        while let Some(c) = self.peek(0) {
            self.at += 1;
            out.push(c);
            match c {
                '{' => depth += 1,
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return;
                    }
                },
                _ => {},
            }
        }
    }

    pub(super) fn substitution(&mut self) {
        let inner = self.take_group();
        self.nest(&inner);
    }

    pub(super) fn nest(&mut self, inner: &str) {
        self.word.push_str("$(…)");
        self.has_word = true;
        match Self::new(inner, self.dialect, self.depth + 1).run() {
            Some(found) => self.nested.extend(found),
            None => self.too_deep = true,
        }
    }

    pub(super) fn separator(&mut self, width: usize) {
        self.end_segment();
        self.at += width;
    }
}
