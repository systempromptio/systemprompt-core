//! Redirections and here-documents for the lexer: a redirect's target or a
//! here-document's delimiter is taken from the next word, and a body that
//! expands is scanned for substitutions line by line until its delimiter.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::lexer::Lexer;

#[derive(Debug)]
pub(super) struct HereDocument {
    pub(super) delimiter: String,
    pub(super) strip_tabs: bool,
    pub(super) expands: bool,
}


impl Lexer {
    pub(super) fn here_bodies(&mut self) {
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

    pub(super) fn expansions(&mut self, text: &str) {
        let mut inner = Self::new(text, self.dialect, self.depth + 1);
        inner.double_body(false);
        if inner.unanalysable {
            self.unanalysable = true;
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
}
