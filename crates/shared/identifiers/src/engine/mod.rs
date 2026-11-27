//! The workspace's own typed identifiers.
//!
//! Minted identities ([`ProjectId`], [`SessionId`], [`MessageId`], [`PartId`],
//! [`JobId`], [`PermissionId`], [`QuestionId`]) are prefixed
//! strings that sort in creation order. A [`CallId`] is the opaque id a model
//! gives a tool call; it is validated as non-empty and otherwise kept verbatim.
//! An [`ExtensionId`] names an extension and is checked against its own shape.
//! Every type validates on `parse` / `try_new`, `FromStr` and `Deserialize`,
//! and none converts from a raw `String` without validation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod call;
mod clock;
mod error;
mod extension;
mod prefixed;

pub use call::CallId;
pub use clock::now_ms;
pub use error::IdError;
pub use extension::{ExtensionId, ExtensionLiteral};
pub use prefixed::{JobId, MessageId, PartId, PermissionId, ProjectId, QuestionId, SessionId};
