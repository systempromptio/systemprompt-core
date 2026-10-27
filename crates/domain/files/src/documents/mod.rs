//! Shared document store: folders, documents, content-addressed revisions,
//! subject-keyed grants, locks, presence, edit sessions, codecs and templates.
//!
//! [`blob_store`] stores revision bytes by content hash over `FileStorage`,
//! and [`jobs`] collects unreferenced blobs and expired presence rows.
//! [`models`] holds the row and request types; [`repository`] holds one
//! repository per table, built once into [`repository::DocumentRepositories`].
//! [`codecs`] holds the core [`DocumentCodec`] implementations and the
//! [`CodecRegistry`] a composition root builds once to pick the codec for each
//! format. [`templates`] holds the [`TemplateRegistry`] that merges template
//! providers with the services catalog, and the core blank templates.
//!
//! [`TemplateRegistry`]: templates::TemplateRegistry
//! [`DocumentCodec`]: systemprompt_provider_contracts::documents::DocumentCodec
//! [`CodecRegistry`]: codecs::CodecRegistry
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod blob_store;
pub mod codecs;
pub mod jobs;
pub mod models;
pub mod repository;
pub mod templates;
