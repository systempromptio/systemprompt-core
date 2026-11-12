//! Unit tests for systemprompt-storage crate
//!
//! Tests cover:
//! - LocalFileStorage round trips and path validation
//! - build_file_storage backend selection
//! - GcsFileStorage against a mocked JSON API and the metadata token cache
//! - S3FileStorage against a mocked S3 endpoint
//! - probe_shared_mount marker exchange between instance ids

#![allow(clippy::all)]

#[cfg(test)]
mod gcs_resumable;
#[cfg(test)]
mod gcs_storage;
#[cfg(test)]
mod local_storage;
#[cfg(test)]
mod local_storage_failures;
#[cfg(all(test, unix))]
mod local_storage_symlinks;
#[cfg(test)]
mod ranged_reads;
#[cfg(test)]
mod s3_multipart;
#[cfg(test)]
mod s3_storage;
#[cfg(test)]
mod shared_mount_probe;

#[cfg(test)]
mod storage_edges;
#[cfg(test)]
mod support;
