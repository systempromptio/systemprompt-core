//! In-place scalar update of a YAML values file.
//!
//! Ops repositories are reviewed as text, so the digest rewrite edits one
//! line and leaves every other byte — comments, ordering, quoting style —
//! as authored. It understands block mappings only: a path through a
//! sequence or a flow mapping is refused rather than guessed at. A missing
//! key is created at the end of its parent mapping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValuesFileError {
    #[error("the key path is empty or has an empty segment")]
    EmptyPath,
    #[error("'{path}' holds a sequence, not a mapping")]
    Sequence { path: String },
    #[error("'{path}' is a mapping, not a scalar")]
    Mapping { path: String },
    #[error("'{path}' is a scalar, not a mapping")]
    Scalar { path: String },
}

struct KeyLine<'a> {
    raw_key: &'a str,
    key: &'a str,
    rest: &'a str,
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_content(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && !trimmed.starts_with('#')
}

fn key_line(line: &str) -> Option<KeyLine<'_>> {
    let body = line.trim_start_matches(' ');
    if body.starts_with('-') || body.starts_with('{') || body.starts_with('[') {
        return None;
    }
    let colon = body
        .char_indices()
        .find(|&(i, c)| c == ':' && body[i + 1..].chars().next().is_none_or(|n| n == ' '))
        .map(|(i, _)| i)?;
    let raw_key = &body[..colon];
    let key = raw_key
        .strip_prefix('"')
        .and_then(|k| k.strip_suffix('"'))
        .or_else(|| {
            raw_key
                .strip_prefix('\'')
                .and_then(|k| k.strip_suffix('\''))
        })
        .unwrap_or(raw_key);
    Some(KeyLine {
        raw_key,
        key,
        rest: body[colon + 1..].trim(),
    })
}

fn trailing_comment(rest: &str) -> &str {
    if rest.starts_with('#') {
        return rest;
    }
    rest.find(" #").map_or("", |pos| &rest[pos..])
}

fn holds_value(rest: &str) -> bool {
    !rest.is_empty() && !rest.starts_with('#')
}

fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn block_end(lines: &[&str], from: usize, owner_indent: usize) -> usize {
    (from..lines.len())
        .find(|&i| is_content(lines[i]) && indent_of(lines[i]) <= owner_indent)
        .unwrap_or(lines.len())
}

struct Block {
    start: usize,
    end: usize,
    owner_indent: Option<usize>,
}

pub fn set_yaml_scalar(
    content: &str,
    key_path: &[&str],
    value: &str,
) -> Result<String, ValuesFileError> {
    if key_path.is_empty() || key_path.iter().any(|segment| segment.is_empty()) {
        return Err(ValuesFileError::EmptyPath);
    }
    let view: Vec<&str> = content.lines().collect();
    let mut lines: Vec<String> = view.iter().map(|line| (*line).to_owned()).collect();
    let mut block = Block {
        start: 0,
        end: view.len(),
        owner_indent: None,
    };

    for (depth, segment) in key_path.iter().enumerate() {
        let child_indent = first_child_indent(&view, &block, &key_path[..depth])?;
        let Some(index) = find_key(&view, &block, child_indent, segment) else {
            let base =
                child_indent.unwrap_or_else(|| block.owner_indent.map_or(0, |owner| owner + 2));
            let insert_at = insert_point(&view, &block);
            insert_path(&mut lines, insert_at, base, &key_path[depth..], value);
            return Ok(render(&lines, content));
        };

        let line_indent = indent_of(view[index]);
        let rest = key_line(view[index]).map_or("", |parsed| parsed.rest);
        let child_end = block_end(&view, index + 1, line_indent);

        if depth + 1 == key_path.len() {
            let has_children = view[index + 1..child_end].iter().any(|l| is_content(l));
            if has_children && !holds_value(rest) {
                return Err(ValuesFileError::Mapping {
                    path: key_path.join("."),
                });
            }
            lines[index] = replaced_line(view[index], value);
            return Ok(render(&lines, content));
        }

        if holds_value(rest) {
            return Err(ValuesFileError::Scalar {
                path: key_path[..=depth].join("."),
            });
        }
        block = Block {
            start: index + 1,
            end: child_end,
            owner_indent: Some(line_indent),
        };
    }
    Err(ValuesFileError::EmptyPath)
}

fn first_child_indent(
    view: &[&str],
    block: &Block,
    parent_path: &[&str],
) -> Result<Option<usize>, ValuesFileError> {
    let Some(first) = view[block.start..block.end]
        .iter()
        .find(|line| is_content(line))
    else {
        return Ok(None);
    };
    if first.trim_start().starts_with('-') {
        return Err(ValuesFileError::Sequence {
            path: parent_path.join("."),
        });
    }
    Ok(Some(indent_of(first)))
}

fn find_key(
    view: &[&str],
    block: &Block,
    child_indent: Option<usize>,
    segment: &str,
) -> Option<usize> {
    (block.start..block.end).find(|&i| {
        is_content(view[i])
            && Some(indent_of(view[i])) == child_indent
            && key_line(view[i]).is_some_and(|k| k.key == segment)
    })
}

fn insert_point(view: &[&str], block: &Block) -> usize {
    (block.start..block.end)
        .rev()
        .find(|&i| is_content(view[i]))
        .map_or(block.start, |i| i + 1)
}

fn insert_path(lines: &mut Vec<String>, insert_at: usize, base: usize, keys: &[&str], value: &str) {
    for (offset, key) in keys.iter().enumerate() {
        let pad = " ".repeat(base + offset * 2);
        let line = if offset + 1 == keys.len() {
            format!("{pad}{key}: {}", quote(value))
        } else {
            format!("{pad}{key}:")
        };
        lines.insert(insert_at + offset, line);
    }
}

fn replaced_line(line: &str, value: &str) -> String {
    let indent = indent_of(line);
    let Some(parsed) = key_line(line) else {
        return line.to_owned();
    };
    let comment = trailing_comment(parsed.rest);
    let separator = if comment.is_empty() || comment.starts_with(' ') {
        ""
    } else {
        " "
    };
    format!(
        "{}{}: {}{separator}{comment}",
        " ".repeat(indent),
        parsed.raw_key,
        quote(value)
    )
}

fn render(lines: &[String], original: &str) -> String {
    let mut out = lines.join("\n");
    if original.ends_with('\n') || original.is_empty() {
        out.push('\n');
    }
    out
}
