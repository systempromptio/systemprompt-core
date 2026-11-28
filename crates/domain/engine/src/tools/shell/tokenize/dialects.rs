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
            '$' if next == Some('\'') => self.ansi_c(),
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
            '$' if next == Some('(') && self.peek(2) == Some('(') => self.arithmetic(),
            '$' if next == Some('(') => {
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
            '<' if next == Some('<') && self.peek(2) != Some('<') => self.here_document(),
            '>' | '<' => self.redirect(),
            _ => {
                self.push(c);
                self.at += 1;
            },
        }
    }

    fn ansi_c(&mut self) {
        self.mark_quoted();
        self.at += 2;
        self.has_word = true;
        while let Some(c) = self.peek(0) {
            self.at += 1;
            match c {
                '\'' => return,
                '\\' => self.ansi_c_escape(),
                _ => self.word.push(c),
            }
        }
    }

    fn ansi_c_escape(&mut self) {
        let Some(c) = self.peek(0) else {
            self.word.push('\\');
            return;
        };
        self.at += 1;
        let simple = match c {
            'a' => Some('\u{7}'),
            'b' => Some('\u{8}'),
            'e' | 'E' => Some('\u{1b}'),
            'f' => Some('\u{c}'),
            'n' => Some('\n'),
            'r' => Some('\r'),
            't' => Some('\t'),
            'v' => Some('\u{b}'),
            '\\' | '\'' | '"' | '?' => Some(c),
            _ => None,
        };
        if let Some(decoded) = simple {
            self.word.push(decoded);
            return;
        }
        let (radix, most) = match c {
            '0'..='7' => {
                self.at -= 1;
                (8, 3)
            },
            'x' => (16, 2),
            'u' => (16, 4),
            'U' => (16, 8),
            'c' => {
                if let Some(control) = self.peek(0) {
                    self.at += 1;
                    self.word.push(char::from(
                        u8::try_from(u32::from(control) & 0x1f).unwrap_or(0),
                    ));
                }
                return;
            },
            _ => {
                self.word.push('\\');
                self.word.push(c);
                return;
            },
        };
        let mut value = 0u32;
        let mut digits = 0;
        while digits < most
            && let Some(digit) = self.peek(0).and_then(|d| d.to_digit(radix))
        {
            value = value * radix + digit;
            digits += 1;
            self.at += 1;
        }
        match char::from_u32(value) {
            Some(decoded) if digits > 0 => self.word.push(decoded),
            _ => {
                self.word.push('\\');
                self.word.push(c);
            },
        }
    }

    pub(super) fn posix_double(&mut self) {
        self.mark_quoted();
        self.at += 1;
        self.has_word = true;
        self.double_body(true);
    }

    pub(super) fn double_body(&mut self, closing: bool) {
        while let Some(c) = self.peek(0) {
            match c {
                '"' if closing => {
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
                '$' if self.peek(1) == Some('(') && self.peek(2) == Some('(') => self.arithmetic(),
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

    fn arithmetic(&mut self) {
        self.at += 1;
        let inner = self.take_group();
        self.word.push_str("$(");
        self.word.push_str(&inner);
        self.word.push(')');
        self.has_word = true;
        self.expansions(&inner);
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
