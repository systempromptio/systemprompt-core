//! The rule that bounds a row nested in another owner's namespace, and the
//! compile-time check of the reserved table against it.
//!
//! A row nested in a namespace another owner holds (`/api/v1/mcp/registry`
//! inside the kernel's `/api/v1/mcp`, or one member's row inside another
//! member's) is `Exact` or `Subtree`, never `Prefix`: a prefix row would
//! hand its holder every path whose text continues it
//! (`/api/v1/mcp/registryx`), beside routes the enclosing owner serves. Rows
//! of one owner nested in each other are that owner's own business.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{CoreId, RESERVED_PATHS, ReservedPath, ReservedReach};

const _: () = assert!(
    nested_rows_are_bounded(RESERVED_PATHS),
    "a row nested under another owner's row must be Exact or Subtree, never Prefix"
);

const fn bytes_start_with(path: &[u8], prefix: &[u8]) -> bool {
    if prefix.len() > path.len() {
        return false;
    }
    let mut index = 0;
    while index < prefix.len() {
        if path[index] != prefix[index] {
            return false;
        }
        index += 1;
    }
    true
}

const fn encloses_text(outer: &ReservedPath, inner: &ReservedPath) -> bool {
    let (outer_path, inner_path) = (outer.path.as_bytes(), inner.path.as_bytes());
    if outer_path.len() == inner_path.len() || !bytes_start_with(inner_path, outer_path) {
        return false;
    }
    match outer.reach {
        ReservedReach::Prefix => true,
        ReservedReach::Subtree => {
            inner_path[outer_path.len()] == b'/' || outer_path[outer_path.len() - 1] == b'/'
        },
        ReservedReach::Exact => false,
    }
}

const fn same_owner(a: CoreId, b: CoreId) -> bool {
    a as u8 == b as u8
}

#[must_use]
pub const fn nested_under_another_owner(entry: &ReservedPath, table: &[ReservedPath]) -> bool {
    let mut index = 0;
    while index < table.len() {
        let outer = &table[index];
        if !same_owner(outer.owner, entry.owner) && encloses_text(outer, entry) {
            return true;
        }
        index += 1;
    }
    false
}

#[must_use]
pub const fn nested_rows_are_bounded(table: &[ReservedPath]) -> bool {
    let mut index = 0;
    while index < table.len() {
        let entry = &table[index];
        if matches!(entry.reach, ReservedReach::Prefix) && nested_under_another_owner(entry, table)
        {
            return false;
        }
        index += 1;
    }
    true
}
