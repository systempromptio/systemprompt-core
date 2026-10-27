//! Document and folder authorization.
//!
//! [`DocumentAccessService::authorize`] answers one question in two steps,
//! and both must pass:
//!
//! 1. **Grants.** The owner of a document or folder holds `own`. Everyone else
//!    holds the highest unexpired grant naming one of their subject pairs
//!    (`user`, each role, and every value of every registered subject
//!    dimension) on the document, on its folder or on any ancestor folder: a
//!    folder grant covers its whole subtree. Creating a folder grants its
//!    creator `own` on it, so the subtree stays theirs after documents from
//!    others land in it. The action's required level must not exceed the level
//!    held. Administrators are not special-cased: they reach documents through
//!    grants to their role like anyone else.
//! 2. **Policy.** The authz hook must allow an [`AuthzRequest`] whose entity is
//!    the nearest folder and whose context is `document.access`. Documents and
//!    folders at the root are checked against the root folder entity
//!    ([`root_folder_entity`]), so an operator can govern the root like any
//!    folder. [`DocumentAccessService::probe`] asks the same question through
//!    the hook's `probe`, which writes no audit row, for listings.
//!
//! The root is not an object and holds no grants: any authenticated user may
//! create documents and folders there (subject to the hook), and what they
//! create is visible only to its owner until it is shared.
//!
//! Every step fails closed: a subject provider error, an unknown target or a
//! hook deny is a refusal, never a partial answer.
//!
//! [`AuthzRequest`]: systemprompt_governance::authz::AuthzRequest
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod catalog;
mod resolve;
mod service;
mod subjects;

pub use service::{AccessTarget, DocumentAccessService, Granted, root_folder_entity};
