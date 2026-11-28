//! A pragmatic command splitter for the shell permission check. It finds the
//! separate commands in a line (split on pipes, `&&`, `||`, `;`, newlines,
//! groupings and command substitutions) and the words that look like paths, for
//! POSIX shells, `PowerShell` and cmd. It is not a parser: what it cannot
//! follow stays inside a word, which makes the check ask rather than wave a
//! command through. A line nested too deeply to follow yields no segment,
//! so the check asks about the whole line as written with no "always"
//! pattern.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod dialects;
mod lexer;

use self::lexer::Lexer;

/// The quoting and separator rules a command line follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    Posix,
    PowerShell,
    Cmd,
}

/// One simple command: its words with quotes removed, and the targets of its
/// redirections.
///
/// `command_words` skips variable assignments and shell keywords and is empty
/// for an expression line; `head` is the command name normalised for matching
/// (lower case without `.exe` for `PowerShell` and cmd); `prefix` adds the
/// subcommand words that matter for approval, such as `git commit` or `npm run
/// build`; `path_words` are the words and redirect targets that look like
/// filesystem paths, cut at the first glob character.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Segment {
    pub words: Vec<String>,
    pub redirects: Vec<String>,
    quoted_names: Vec<usize>,
}

const POSIX_SKIPPED: [&str; 15] = [
    "{", "}", "!", "if", "then", "else", "elif", "fi", "do", "done", "while", "until", "time",
    "exec", "coproc",
];
const NULL_DEVICES: [&str; 5] = ["/dev/null", "/dev/stdout", "/dev/stderr", "nul", "$null"];

const ARITY: [(&str, usize); 41] = [
    ("git", 2),
    ("npm", 2),
    ("npm run", 3),
    ("pnpm", 2),
    ("pnpm run", 3),
    ("yarn", 2),
    ("yarn run", 3),
    ("bun", 2),
    ("bun run", 3),
    ("npx", 2),
    ("deno", 2),
    ("cargo", 2),
    ("rustup", 2),
    ("go", 2),
    ("dotnet", 2),
    ("docker", 2),
    ("docker compose", 3),
    ("kubectl", 2),
    ("helm", 2),
    ("terraform", 2),
    ("gh", 3),
    ("az", 3),
    ("aws", 3),
    ("gcloud", 3),
    ("pip", 2),
    ("pip3", 2),
    ("uv", 2),
    ("poetry", 2),
    ("python -m", 3),
    ("python3 -m", 3),
    ("py -m", 3),
    ("winget", 2),
    ("choco", 2),
    ("scoop", 2),
    ("brew", 2),
    ("apt", 2),
    ("apt-get", 2),
    ("make", 2),
    ("mvn", 2),
    ("gradle", 2),
    ("systemctl", 2),
];

impl Segment {
    pub fn command_words(&self, dialect: Dialect) -> &[String] {
        let words = &self.words;
        let mut start = 0;
        while let Some(word) = words.get(start) {
            match dialect {
                Dialect::Posix
                    if (is_assignment(word) && !self.quoted_names.contains(&start))
                        || POSIX_SKIPPED.contains(&word.as_str()) =>
                {
                    start += 1;
                },
                Dialect::PowerShell if word.starts_with('$') => {
                    if words.get(start + 1).is_some_and(|next| is_ps_assign(next)) {
                        start += 2;
                    } else {
                        return &[];
                    }
                },
                _ => break,
            }
        }
        &words[start..]
    }

    pub fn head(&self, dialect: Dialect) -> Option<String> {
        self.command_words(dialect)
            .first()
            .map(|word| normalize_head(word, dialect))
    }

    pub fn text(&self, dialect: Dialect) -> String {
        let words = self.command_words(dialect);
        words
            .iter()
            .enumerate()
            .map(|(index, word)| {
                if index == 0 {
                    normalize_head(word, dialect)
                } else {
                    word.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn prefix(&self, dialect: Dialect) -> String {
        let words: Vec<String> = self
            .command_words(dialect)
            .iter()
            .enumerate()
            .map(|(index, word)| {
                if index == 0 {
                    normalize_head(word, dialect)
                } else {
                    word.clone()
                }
            })
            .collect();
        let mut arity = 1;
        let mut matched = 0;
        for (prefix, count) in ARITY {
            let parts: Vec<&str> = prefix.split(' ').collect();
            if parts.len() > matched
                && parts.len() <= words.len()
                && parts
                    .iter()
                    .zip(&words)
                    .all(|(part, word)| *part == word.as_str())
            {
                arity = count;
                matched = parts.len();
            }
        }
        let mut taken: Vec<&str> = Vec::new();
        for (index, word) in words.iter().enumerate().take(arity) {
            if index > 0 && word.starts_with('-') && index >= matched {
                break;
            }
            taken.push(word);
        }
        taken.join(" ")
    }

    pub fn path_words(&self, dialect: Dialect) -> Vec<String> {
        self.words
            .iter()
            .skip(1)
            .chain(&self.redirects)
            .filter_map(|word| path_like(word, dialect))
            .collect()
    }
}

fn is_assignment(word: &str) -> bool {
    match word.split_once('=') {
        Some((name, _)) => {
            !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !name.starts_with(|c: char| c.is_ascii_digit())
        },
        None => false,
    }
}

fn is_ps_assign(word: &str) -> bool {
    matches!(word, "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "??=")
}

pub(crate) fn normalize_head(word: &str, dialect: Dialect) -> String {
    match dialect {
        Dialect::Posix => word.to_owned(),
        Dialect::PowerShell | Dialect::Cmd => {
            let lower = word.to_lowercase();
            match lower.strip_suffix(".exe") {
                Some(stem) if !stem.is_empty() => stem.to_owned(),
                _ => lower,
            }
        },
    }
}

const fn has_drive(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn path_like(word: &str, dialect: Dialect) -> Option<String> {
    let mut text = word.trim();
    if text.is_empty() || text.contains("://") || text.contains('$') || text.contains('`') {
        return None;
    }
    if dialect == Dialect::Cmd && text.contains('%') {
        return None;
    }
    if NULL_DEVICES
        .iter()
        .any(|device| text.eq_ignore_ascii_case(device))
    {
        return None;
    }
    if text.starts_with('-') {
        let (_, value) = text.split_once('=').or_else(|| {
            if dialect == Dialect::PowerShell {
                text.split_once(':')
            } else {
                None
            }
        })?;
        text = value.trim_matches(['"', '\'']);
    }
    if dialect == Dialect::Cmd
        && text
            .strip_prefix('/')
            .is_some_and(|rest| !rest.contains(['/', '\\']))
    {
        return None;
    }
    let cut = text.find(['*', '?', '[', '{']).unwrap_or(text.len());
    let base = &text[..cut];
    let looks = base.starts_with(['/', '\\', '~'])
        || base.starts_with("..")
        || base.contains('/')
        || base.contains('\\')
        || has_drive(base);
    (looks && !base.is_empty()).then(|| base.to_owned())
}

pub fn split_commands(command: &str, dialect: Dialect) -> Vec<Segment> {
    Lexer::new(command, dialect, 0).run().unwrap_or_default()
}
