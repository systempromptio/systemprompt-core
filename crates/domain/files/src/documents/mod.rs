//! Shared document store: folders, documents, content-addressed revisions,
//! subject-keyed grants, locks, presence, edit sessions, codecs and templates.
//!
//! [`access`] decides who may do what ([`access::DocumentAccessService`]);
//! [`service`] is the application service ([`service::DocumentService`]) that
//! runs every operation through it, with the pure commit logic in
//! [`revision`] and [`nodes`] (stable node ids) and the AG-UI events in
//! [`events`]. [`error`] is the one error type.
//! [`blob_store`] stores revision bytes by content hash over `FileStorage`,
//! and [`jobs`] collects unreferenced blobs and expired presence rows.
//! [`models`] holds the row and request types; [`repository`] holds one
//! repository per table, built once into [`repository::DocumentRepositories`].
//! [`codecs`] holds the core [`DocumentCodec`] implementations and the
//! [`CodecRegistry`] a composition root builds once to pick the codec for each
//! format. [`templates`] holds the [`TemplateRegistry`] that merges template
//! providers with the services catalog, and the core blank templates.
//!
//! With the `collab` feature, `collab` co-edits HTML and Markdown documents
//! live over a Loro CRDT.
//!
//! [`TemplateRegistry`]: templates::TemplateRegistry
//! [`DocumentCodec`]: systemprompt_provider_contracts::documents::DocumentCodec
//! [`CodecRegistry`]: codecs::CodecRegistry
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod access;
pub mod blob_store;
pub mod codecs;
#[cfg(feature = "collab")]
pub mod collab;
pub mod engine;
pub mod error;
pub mod events;
pub mod jobs;
pub mod models;
pub mod nodes;
pub mod repository;
pub mod revision;
pub mod service;
pub mod templates;

pub use access::{
    AccessCheck, AccessTarget, DocumentAccessRepositories, DocumentAccessService, Granted,
    SubjectDirectory, SubjectDirectoryError,
};
pub use error::DocumentsError;
pub use events::DocumentEvents;
pub use service::{DocumentService, DocumentServiceParts};
