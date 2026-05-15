//! Byte-offset → `(line, column)` mapping for schema-linter diagnostics.

/// Statement position and source label, threaded through the per-node checks
/// so they stay under the argument-count limit.
pub(super) struct StmtLoc<'a> {
    pub line: u32,
    pub col: u32,
    pub source: &'a str,
}

/// Advance past leading whitespace and `--` / `/* */` comments so a statement's
/// reported position points at its first significant token rather than at the
/// trailing comment of the previous statement.
pub(super) fn stmt_start_offset(sql: &str, start: usize) -> usize {
    let bytes = sql.as_bytes();
    let mut i = start;
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            let mut depth = 1u32;
            while i + 1 < bytes.len() && depth > 0 {
                if bytes[i] == b'/' && bytes[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        break;
    }
    i
}

pub(super) struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub(super) fn new(text: &str) -> Self {
        let mut line_starts = vec![0usize];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self { line_starts }
    }

    pub(super) fn position(&self, byte_offset: usize) -> (u32, u32) {
        let line_idx = match self.line_starts.binary_search(&byte_offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let line_start = self.line_starts[line_idx];
        let line = (line_idx as u32) + 1;
        let col = ((byte_offset - line_start) as u32) + 1;
        (line, col)
    }
}
