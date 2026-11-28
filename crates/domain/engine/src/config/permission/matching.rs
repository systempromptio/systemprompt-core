//! Wildcard matching of rule patterns against permissions and subjects.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use super::platform;
use crate::config::paths::display_slash;

pub fn inside(dir: &Path) -> String {
    let base = display_slash(dir);
    format!("{}/*", base.trim_end_matches('/'))
}

pub fn wildcard(pattern: &str, subject: &str) -> bool {
    let subject = normalize(subject);
    if let Some(head) = pattern.strip_suffix(" *")
        && glob(&normalize(head), &subject)
    {
        return true;
    }
    glob(&normalize(pattern), &subject)
}

fn normalize(text: &str) -> Vec<char> {
    platform::fold(text.chars().map(|c| if c == '\\' { '/' } else { c }))
}

fn glob(pattern: &[char], subject: &[char]) -> bool {
    let (mut p, mut s) = (0, 0);
    let mut backtrack: Option<(usize, usize)> = None;
    while s < subject.len() {
        match pattern.get(p) {
            Some('*') => {
                backtrack = Some((p, s));
                p += 1;
            },
            Some(&c) if c == '?' || c == subject[s] => {
                p += 1;
                s += 1;
            },
            _ => match backtrack {
                Some((star, from)) => {
                    p = star + 1;
                    s = from + 1;
                    backtrack = Some((star, from + 1));
                },
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}
