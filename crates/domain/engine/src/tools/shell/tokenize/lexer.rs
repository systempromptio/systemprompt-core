//! The character-level lexer behind `split_commands`: words, quotes,
//! redirections, separators and substitutions, with the per-dialect rules in a
//! sibling module.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{Dialect, Segment};

const MAX_DEPTH: usize = 64;

/// A here-document whose delimiter has been read and whose body starts after
/// the next line feed.
#[derive(Debug)]
pub(super) struct HereDocument {
    delimiter: String,
    strip_tabs: bool,
    expands: bool,
}

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

    pub(super) fn mark_quoted(&mut self) {
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

    fn here_bodies(&mut self) {
        for document in std::mem::take(&mut self.here_documents) {
            while self.at < self.chars.len() {
                let start = self.at;
                let end = self.chars[start..]
                    .iter()
                    .position(|&c| c == '\n')
                    .map_or(self.chars.len(), |offset| start + offset);
                self.at = (end + 1).min(self.chars.len());
                let line: String = self.chars[start..end].iter().collect();
                let line = line.strip_suffix('\r').unwrap_or(&line);
                let compared = if document.strip_tabs {
                    line.trim_start_matches('\t')
                } else {
                    line
                };
                if compared == document.delimiter {
                    break;
                }
                if document.expands {
                    self.expansions(line);
                }
            }
        }
    }

    /// Scans text where only expansions run, a here-document body or
    /// arithmetic, and keeps the commands of its substitutions.
    pub(super) fn expansions(&mut self, text: &str) {
        let mut inner = Self::new(text, self.dialect, self.depth + 1);
        inner.double_body(false);
        if inner.too_deep {
            self.too_deep = true;
        }
        self.nested.extend(inner.segments);
        self.nested.extend(inner.nested);
    }

    pub(super) fn here_document(&mut self) {
        self.take_descriptor();
        self.at += 2;
        let strip_tabs = self.peek(0) == Some('-');
        if strip_tabs {
            self.at += 1;
        }
        self.here_next = Some(strip_tabs);
    }

    fn take_descriptor(&mut self) {
        let descriptor = self.has_word
            && self.quoted_at.is_none()
            && (self.word.chars().all(|c| c.is_ascii_digit()) || self.word == "*");
        if descriptor {
            self.word.clear();
            self.has_word = false;
        } else {
            self.flush_word();
        }
    }

    pub(super) fn redirect(&mut self) {
        self.take_descriptor();
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
