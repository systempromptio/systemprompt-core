//! Stable node ids: the identity of a block across revisions.
//!
//! Codecs address nodes by position (`body/p[3]`), which shifts when a block
//! is inserted above. The service gives every node of a revision an opaque id
//! `<writer>.<counter>` instead and persists the map per revision. The writer
//! is a short base36 tag of the committing session or execution and the
//! counter rises per writer within a document, so concurrent writers never
//! mint the same id and an id is never reused.
//!
//! A new revision inherits ids from its parent by [`carry_forward`]: blocks
//! with the same kind and hash are aligned by a longest common subsequence,
//! changed blocks between two aligned blocks keep their ids positionally, and
//! whatever is left is minted (inserted) or retired (deleted). Ids a commit
//! deleted by op are retired up front.
//! [`with_ids`] turns a codec projection into the projection the tool surface
//! sees, keeping the codec path in the [`PATH_ATTR`] attribute; [`changes`]
//! diffs two such projections for incremental reads, and [`derive_ops`]
//! expresses an uploaded Office file as edit ops against its base.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod carry;
mod derive;
mod diff;
mod flat;
mod minter;
mod view;
mod written;

pub use carry::{CarryHints, carry_forward, mint_all};
pub use derive::derive_ops;
pub use diff::{ChangeItem, NodeChanges, changes};
pub use flat::{FlatNode, flatten};
pub use minter::{NodeIdMinter, writer_tag};
pub use view::{PATH_ATTR, path_of, to_paths, with_ids};
pub use written::{created_nodes, written_nodes};
