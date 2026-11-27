//! The character-level lexer behind `split_commands`: words, quotes,
//! redirections, separators and substitutions, with the per-dialect rules in a
//! sibling module.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{Dialect, Segment, split_commands};

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
}

impl Lexer {
    pub(super) fn new(command: &str, dialect: Dialect) -> Self {
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
        }
    }

    pub(super) fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    pub(super) fn push(&mut self, c: char) {
        self.word.push(c);
        self.has_word = true;
    }

    pub(super) fn flush_word(&mut self) {
        if !self.has_word {
            return;
        }
        let word = std::mem::take(&mut self.word);
        if self.redirect_next {
            self.current.redirects.push(word);
            self.redirect_next = false;
        } else {
            self.current.words.push(word);
        }
        self.has_word = false;
    }

    pub(super) fn end_segment(&mut self) {
        self.flush_word();
        self.redirect_next = false;
        let segment = std::mem::take(&mut self.current);
        if !segment.words.is_empty() || !segment.redirects.is_empty() {
            self.segments.push(segment);
        }
    }

    pub(super) fn run(mut self) -> Vec<Segment> {
        while let Some(c) = self.peek(0) {
            match c {
                ' ' | '\t' => {
                    self.flush_word();
                    self.at += 1;
                },
                '\n' | '\r' => {
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
        let mut all = self.segments;
        all.extend(self.nested);
        all
    }

    pub(super) fn skip_comment(&mut self) {
        while let Some(c) = self.peek(0) {
            if c == '\n' {
                break;
            }
            self.at += 1;
        }
    }

    pub(super) fn quoted_literal(&mut self, quote: char, doubled_escape: bool) {
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
        while let Some(c) = self.peek(0) {
            self.at += 1;
            if let Some(open) = quote {
                if c == open {
                    quote = None;
                }
                out.push(c);
                continue;
            }
            match c {
                '\'' | '"' => quote = Some(c),
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
        }
        out
    }

    pub(super) fn substitution(&mut self) {
        let inner = self.take_group();
        self.word.push_str("$(…)");
        self.has_word = true;
        let found = split_commands(&inner, self.dialect);
        self.nested.extend(found);
    }

    pub(super) fn redirect(&mut self) {
        let descriptor =
            self.has_word && (self.word.chars().all(|c| c.is_ascii_digit()) || self.word == "*");
        if descriptor {
            self.word.clear();
            self.has_word = false;
        } else {
            self.flush_word();
        }
        self.at += 1;
        while matches!(self.peek(0), Some('>' | '<' | '|')) {
            self.at += 1;
        }
        if self.peek(0) == Some('&') {
            self.at += 1;
            while self.peek(0).is_some_and(|c| c.is_ascii_digit() || c == '-') {
                self.at += 1;
            }
            return;
        }
        self.redirect_next = true;
    }

    pub(super) fn separator(&mut self, width: usize) {
        self.end_segment();
        self.at += width;
    }
}
