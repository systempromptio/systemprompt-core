//! Signed services bundles: packing, fetching, verifying and composing.
//!
//! A bundle is a gzipped tar carrying `bundle.json` and a `services/` tree,
//! published to any HTTPS location or OCI registry and fetched by an instance
//! at boot. The instance is the verifier: an archive digest pin and an
//! ed25519 signature decide whether the bytes are trusted, per-file checksums
//! decide whether the extraction is intact, and the content hash keys the
//! cache. Nothing here warns and continues.
//!
//! # Modules
//!
//! - [`pack`] — builds a manifest and archive from a services tree.
//! - [`source`] — the HTTPS and OCI transports behind
//!   [`source::BundleFetcher`].
//! - [`verify`] — the trust chain, in order.
//! - [`extract`] — hardened tar extraction shared with the backup path.
//! - [`cache`] — content-addressed on-disk layout and the `current` swap.
//! - [`mod@compose`] — overlaying several bundles with an ownership check.
//! - [`bootstrap`] — the boot path and its failure policy.
//! - [`error`] — [`error::BundleError`] and [`error::VerifyFailure`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod bootstrap;
pub mod cache;
pub mod compose;
pub mod error;
pub mod extract;
pub mod pack;
pub mod source;
pub mod verify;

pub use bootstrap::{ServicesSourceBootstrap, cache_root};
pub use cache::BundleCache;
pub use compose::{BundleMember, compose, composed_hash};
pub use error::{BundleError, BundleResult, VerifyFailure};
pub use extract::{BUNDLE_TREE_PREFIX, ExtractOptions, TarLayout, extract_bytes, extract_tarball};
pub use source::{AnyFetcher, BundleFetcher, FetchedBundle, RemoteRef};
pub use verify::{verify_bundle, verify_extracted};
