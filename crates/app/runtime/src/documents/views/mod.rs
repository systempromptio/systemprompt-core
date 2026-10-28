//! Transport-neutral document views.
//!
//! Each view is assembled once here, as a plain struct or hashlines, and
//! returns `DocumentsError`:
//! the MCP tools and the HTTP routes both call it, so both surfaces answer
//! alike. The read views are outline, text, changes, node, search, meta,
//! range, cells and model; `access_view`, `folder_access_view` and
//! `comments_view` read sharing and comment threads; `proposal_lines`
//! renders what a proposal changes; `edit_ack` renders a landed write's
//! acknowledgement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod access;
mod acks;
mod lines;
mod meta;
mod model;
mod outline;
mod read;
mod records;
mod types;

pub use access::{
    AccessOutput, AccessRequestItem, GrantEventItem, GrantItem, InheritedFrom, access_view,
    folder_access_view,
};
pub use acks::edit_ack;
pub use lines::{
    MAX_LINES, MAX_WRITTEN, RenderedLines, parse_cell, render, split_cell_id, structured_line,
    written_lines,
};
pub use outline::{change_lines, node_lines, outline, proposal_lines, search_lines};
pub use read::read_view;
pub use records::{CommentAnchorItem, CommentItem, CommentOutput, CommentRangeItem, comments_view};
pub use types::{DocumentMeta, EditOutput, EditingNow, NodeRange, ReadInput, ReadOutput, ReadView};
