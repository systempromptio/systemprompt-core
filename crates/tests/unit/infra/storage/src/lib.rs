//! Unit tests for systemprompt-storage crate
//!
//! Tests cover:
//! - LocalFileStorage round trips and path validation
//! - build_file_storage backend selection
//! - probe_shared_mount marker exchange between instance ids

#![allow(clippy::all)]

#[cfg(test)]
mod local_storage;
#[cfg(test)]
mod shared_mount_probe;
