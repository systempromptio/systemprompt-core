//! The dialect rules of the command lexer: POSIX shells, `PowerShell` and cmd
//! differ in quoting, escapes, comments and which characters separate commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::lexer::Lexer;

impl Lexer {
    pub(super) fn posix(&mut self, c: char) {
        let next = self.peek(1);
        match c {
            '\\' => {
                self.mark_quoted();
                self.at += 2;
                if let Some(escaped) = next
                    && escaped != '\n'
                {
                    self.push(escaped);
                }
            },
            '\'' => self.quoted_literal('\'', false),
            '"' => self.posix_double(),
            '`' => {
                self.at += 1;
                let mut inner = String::new();
                while let Some(c) = self.peek(0) {
                    self.at += 1;
                    if c == '`' {
                        break;
                    }
                    inner.push(c);
                }
                self.nest(&inner);
            },
            '$' if next == Some('(') && self.peek(2) != Some('(') => {
                self.at += 1;
                self.substitution();
            },
            '#' if !self.has_word => self.skip_comment(),
            '|' => self.separator(if matches!(next, Some('|' | '&')) {
                2
            } else {
                1
            }),
            '&' if next == Some('&') => self.separator(2),
            '&' if next == Some('>') => {
                self.flush_word();
                self.at += 1;
                self.redirect();
            },
            '&' | ';' | '(' | ')' => {
                self.separator(if c == ';' && next == Some(';') { 2 } else { 1 });
            },
            '>' | '<' => self.redirect(),
            _ => {
                self.push(c);
                self.at += 1;
            },
        }
    }

    pub(super) fn posix_double(&mut self) {
        self.mark_quoted();
        self.at += 1;
        self.has_word = true;
        while let Some(c) = self.peek(0) {
            match c {
                '"' => {
                    self.at += 1;
                    return;
                },
                '\\' if matches!(self.peek(1), Some('"' | '\\' | '$' | '`' | '\n')) => {
                    if let Some(escaped) = self.peek(1)
                        && escaped != '\n'
                    {
                        self.word.push(escaped);
                    }
                    self.at += 2;
                },
                '$' if self.peek(1) == Some('(') && self.peek(2) != Some('(') => {
                    self.at += 1;
                    self.substitution();
                },
                _ => {
                    self.word.push(c);
                    self.at += 1;
                },
            }
        }
    }

    pub(super) fn powershell(&mut self, c: char) {
        let next = self.peek(1);
        match c {
            '`' => {
                self.mark_quoted();
                self.at += 2;
                if let Some(escaped) = next
                    && escaped != '\n'
                {
                    self.push(escaped);
                }
            },
            '\'' => self.quoted_literal('\'', true),
            '"' => self.powershell_double(),
            '$' | '@' if next == Some('(') => {
                self.at += 1;
                self.substitution();
            },
            '<' if next == Some('#') => {
                self.at += 2;
                while let Some(c) = self.peek(0) {
                    self.at += 1;
                    if c == '#' && self.peek(0) == Some('>') {
                        self.at += 1;
                        break;
                    }
                }
            },
            '#' if !self.has_word => self.skip_comment(),
            '|' => self.separator(if next == Some('|') { 2 } else { 1 }),
            '&' if next == Some('&') => self.separator(2),
            '&' if self.current.words.is_empty() && !self.has_word => self.at += 1,
            '&' | ';' | '{' | '}' | '(' | ')' => self.separator(1),
            '>' | '<' => self.redirect(),
            _ => {
                self.push(c);
                self.at += 1;
            },
        }
    }

    pub(super) fn powershell_double(&mut self) {
        self.mark_quoted();
        self.at += 1;
        self.has_word = true;
        while let Some(c) = self.peek(0) {
            match c {
                '"' if self.peek(1) == Some('"') => {
                    self.word.push('"');
                    self.at += 2;
                },
                '"' => {
                    self.at += 1;
                    return;
                },
                '`' => {
                    if let Some(escaped) = self.peek(1) {
                        self.word.push(escaped);
                    }
                    self.at += 2;
                },
                '$' if self.peek(1) == Some('(') => {
                    self.at += 1;
                    self.substitution();
                },
                _ => {
                    self.word.push(c);
                    self.at += 1;
                },
            }
        }
    }

    pub(super) fn cmd(&mut self, c: char) {
        let next = self.peek(1);
        match c {
            '^' => {
                self.mark_quoted();
                self.at += 2;
                if let Some(escaped) = next
                    && escaped != '\n'
                {
                    self.push(escaped);
                }
            },
            '"' => self.quoted_literal('"', false),
            '@' if self.current.words.is_empty() && !self.has_word => self.at += 1,
            '&' | '|' => self.separator(if next == Some(c) { 2 } else { 1 }),
            '(' | ')' => self.separator(1),
            '>' | '<' => self.redirect(),
            _ => {
                self.push(c);
                self.at += 1;
            },
        }
    }
}
