//! Parsing the patch envelope into file operations: added files with their
//! lines, deletions, and updates made of hunks, each an optional anchor and its
//! kept, removed and added lines.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const BEGIN: &str = "%% begin patch";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FileOp {
    Add {
        path: String,
        content: String,
    },
    Delete {
        path: String,
    },
    Update {
        path: String,
        rename_to: Option<String>,
        hunks: Vec<Hunk>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Hunk {
    pub(super) anchor: Option<String>,
    pub(super) lines: Vec<HunkLine>,
    pub(super) at_end: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum HunkLine {
    Keep(String),
    Remove(String),
    Add(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(super) enum PatchError {
    #[error("the patch must start with the line `%% begin patch`")]
    NoBegin,
    #[error("the patch contains no file changes")]
    Empty,
    #[error("patch line {line}: {problem}")]
    Syntax { line: usize, problem: String },
    #[error("{path}: the lines of hunk {hunk} were not found in the file. Expected:\n{expected}")]
    HunkNotFound {
        path: String,
        hunk: usize,
        expected: String,
    },
    #[error("`%% update {path}` has no hunks")]
    NoHunks { path: String },
    #[error("a hunk in {path} changes nothing; every hunk needs a - or + line")]
    IdleHunk { path: String },
    #[error("{path}: the anchor `{anchor}` of hunk {hunk} was not found")]
    AnchorNotFound {
        path: String,
        hunk: usize,
        anchor: String,
    },
}

fn syntax(line: usize, problem: impl Into<String>) -> PatchError {
    PatchError::Syntax {
        line,
        problem: problem.into(),
    }
}

#[derive(Debug, Default)]
struct Parser {
    ops: Vec<FileOp>,
    current: Option<FileOp>,
}

pub(super) fn parse(text: &str) -> Result<Vec<FileOp>, PatchError> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.trim() == BEGIN)
        .ok_or(PatchError::NoBegin)?;
    let mut parser = Parser::default();
    for (index, raw) in lines.iter().enumerate().skip(start + 1) {
        let number = index + 1;
        if let Some(directive) = raw.strip_prefix("%%") {
            if parser.directive(number, directive.trim())? {
                break;
            }
            continue;
        }
        parser.body_line(number, raw)?;
    }
    let mut ops = parser.ops;
    ops.extend(parser.current);
    if ops.is_empty() {
        return Err(PatchError::Empty);
    }
    check_updates(&ops)?;
    Ok(ops)
}

fn check_updates(ops: &[FileOp]) -> Result<(), PatchError> {
    for op in ops {
        let FileOp::Update {
            path,
            rename_to,
            hunks,
        } = op
        else {
            continue;
        };
        if hunks.is_empty() && rename_to.is_none() {
            return Err(PatchError::NoHunks { path: path.clone() });
        }
        let idle = |hunk: &Hunk| {
            hunk.lines
                .iter()
                .all(|line| matches!(line, HunkLine::Keep(_)))
        };
        if hunks.iter().any(idle) {
            return Err(PatchError::IdleHunk { path: path.clone() });
        }
    }
    Ok(())
}

impl Parser {
    fn directive(&mut self, number: usize, directive: &str) -> Result<bool, PatchError> {
        let (word, arg) = match directive.split_once(char::is_whitespace) {
            Some((word, arg)) => (word, arg.trim()),
            None => (directive, ""),
        };
        match (word, arg) {
            ("end", "patch") => return Ok(true),
            ("end", "of file") => self.end_of_file(number)?,
            ("add" | "delete" | "update", path) => self.open(number, word, path)?,
            ("rename-to", path) => self.rename(number, path)?,
            _ => {
                return Err(syntax(
                    number,
                    format!("unknown directive `%% {directive}`"),
                ));
            },
        }
        Ok(false)
    }

    fn end_of_file(&mut self, number: usize) -> Result<(), PatchError> {
        match &mut self.current {
            Some(FileOp::Update { hunks, .. }) if !hunks.is_empty() => {
                if let Some(last) = hunks.last_mut() {
                    last.at_end = true;
                }
                Ok(())
            },
            _ => Err(syntax(
                number,
                "`%% end of file` must follow a hunk of an update",
            )),
        }
    }

    fn open(&mut self, number: usize, word: &str, path: &str) -> Result<(), PatchError> {
        if path.is_empty() {
            return Err(syntax(number, format!("`%% {word}` needs a file path")));
        }
        self.ops.extend(self.current.take());
        let path = path.to_owned();
        self.current = Some(match word {
            "add" => FileOp::Add {
                path,
                content: String::new(),
            },
            "delete" => FileOp::Delete { path },
            _ => FileOp::Update {
                path,
                rename_to: None,
                hunks: Vec::new(),
            },
        });
        Ok(())
    }

    fn rename(&mut self, number: usize, path: &str) -> Result<(), PatchError> {
        match &mut self.current {
            Some(FileOp::Update {
                rename_to, hunks, ..
            }) if rename_to.is_none() && hunks.is_empty() && !path.is_empty() => {
                *rename_to = Some(path.to_owned());
                Ok(())
            },
            _ => Err(syntax(
                number,
                "`%% rename-to <path>` must come directly after `%% update`",
            )),
        }
    }

    fn body_line(&mut self, number: usize, raw: &str) -> Result<(), PatchError> {
        match &mut self.current {
            None if !raw.trim().is_empty() => Err(syntax(
                number,
                "text outside a file section; start one with %% add, %% update or %% delete",
            )),
            Some(FileOp::Add { content, .. }) => {
                if let Some(rest) = raw.strip_prefix('+') {
                    content.push_str(rest);
                    content.push('\n');
                    Ok(())
                } else if raw.is_empty() {
                    content.push('\n');
                    Ok(())
                } else {
                    Err(syntax(number, "every line of an added file starts with +"))
                }
            },
            Some(FileOp::Delete { .. }) if !raw.trim().is_empty() => {
                Err(syntax(number, "a deleted file takes no lines"))
            },
            Some(FileOp::Update { hunks, .. }) => hunk_line(hunks, number, raw),
            None | Some(FileOp::Delete { .. }) => Ok(()),
        }
    }
}

fn hunk_line(hunks: &mut Vec<Hunk>, number: usize, raw: &str) -> Result<(), PatchError> {
    if let Some(rest) = raw.strip_prefix("@@") {
        let anchor = rest.trim().trim_end_matches("@@").trim();
        let anchor = (!anchor.is_empty()).then(|| anchor.to_owned());
        hunks.push(Hunk {
            anchor,
            ..Hunk::default()
        });
        return Ok(());
    }
    let line = match raw.as_bytes().first() {
        Some(b' ') => HunkLine::Keep(raw[1..].to_owned()),
        Some(b'-') => HunkLine::Remove(raw[1..].to_owned()),
        Some(b'+') => HunkLine::Add(raw[1..].to_owned()),
        None => HunkLine::Keep(String::new()),
        Some(_) => {
            return Err(syntax(
                number,
                "hunk lines start with a space (unchanged), - (removed) or + (added)",
            ));
        },
    };
    if hunks.is_empty() {
        hunks.push(Hunk::default());
    }
    if let Some(last) = hunks.last_mut() {
        last.lines.push(line);
    }
    Ok(())
}
