//! The rule that bounds a member row nested in the kernel's namespace, and the
//! compile-time check of the reserved table against it.
//!
//! A member row nested in the kernel's namespace (`/api/v1/mcp/registry`
//! inside `/api/v1/mcp`) is `Exact` or `Subtree`, never `Prefix`: a prefix
//! row would hand its holder every path whose text continues it
//! (`/api/v1/mcp/registryx`), beside routes the kernel serves.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{CoreId, RESERVED_PATHS, ReservedPath, ReservedReach};

const _: () = assert!(
    nested_rows_are_bounded(RESERVED_PATHS),
    "a member row nested under a kernel row must be Exact or Subtree, never Prefix"
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

#[must_use]
pub const fn nested_under_kernel(entry: &ReservedPath, table: &[ReservedPath]) -> bool {
    let mut index = 0;
    while index < table.len() {
        let outer = &table[index];
        if matches!(outer.owner, CoreId::Kernel) && encloses_text(outer, entry) {
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
        if !matches!(entry.owner, CoreId::Kernel)
            && matches!(entry.reach, ReservedReach::Prefix)
            && nested_under_kernel(entry, table)
        {
            return false;
        }
        index += 1;
    }
    true
}
