//! Storage path layout for uploaded and generated files.
//!
//! `files` under `paths.storage` is the writable layer (uploads, generated
//! images, operator overrides); `shipped_files` is the optional read-only
//! layer an image ships, consulted after it.
//!
//! `data` is the one sanctioned root for runtime state the server writes for
//! itself (the gateway accounting journal), and `data/scratch` the one root
//! for short-lived working trees (Git checkouts of managed sources, import
//! staging), so a read-only root filesystem needs no writable `/tmp`. Both
//! default to `paths.storage/data` because that is the mount every deployment
//! already makes writable; the profile directory is configuration and may be
//! mounted read-only. `paths.data` moves them to a node-local volume when
//! `paths.storage` is shared between replicas.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use super::PathError;
use systemprompt_manifest::paths::constants::storage;
use systemprompt_manifest::profile::PathsConfig;

#[derive(Debug, Clone)]
pub struct StoragePaths {
    root: PathBuf,
    files: PathBuf,
    exports: PathBuf,
    data: PathBuf,
    scratch: PathBuf,
    css: PathBuf,
    js: PathBuf,
    fonts: PathBuf,
    images: PathBuf,
    generated_images: PathBuf,
    logos: PathBuf,
    audio: PathBuf,
    video: PathBuf,
    documents: PathBuf,
    uploads: PathBuf,
    shipped_files: Option<PathBuf>,
}

impl StoragePaths {
    pub fn from_profile(paths: &PathsConfig) -> Result<Self, PathError> {
        let root = paths
            .storage
            .as_ref()
            .ok_or(PathError::NotConfigured { field: "storage" })?;

        let root = PathBuf::from(root);
        let files = root.join(storage::FILES);
        let data = paths
            .data
            .as_ref()
            .map_or_else(|| root.join(storage::DATA), PathBuf::from);

        Ok(Self {
            exports: root.join(storage::EXPORTS),
            scratch: data.join("scratch"),
            data,
            css: files.join("css"),
            js: files.join("js"),
            fonts: files.join("fonts"),
            images: files.join("images"),
            generated_images: files.join("images/generated"),
            logos: files.join("images/logos"),
            audio: files.join("audio"),
            video: files.join("video"),
            documents: files.join("documents"),
            uploads: files.join("uploads"),
            files,
            root,
            shipped_files: paths.shipped_files.as_ref().map(PathBuf::from),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn files(&self) -> &Path {
        &self.files
    }

    pub fn exports(&self) -> &Path {
        &self.exports
    }

    pub fn data(&self) -> &Path {
        &self.data
    }

    pub fn scratch(&self) -> &Path {
        &self.scratch
    }

    pub fn css(&self) -> &Path {
        &self.css
    }

    pub fn js(&self) -> &Path {
        &self.js
    }

    pub fn fonts(&self) -> &Path {
        &self.fonts
    }

    pub fn images(&self) -> &Path {
        &self.images
    }

    pub fn generated_images(&self) -> &Path {
        &self.generated_images
    }

    pub fn logos(&self) -> &Path {
        &self.logos
    }

    pub fn audio(&self) -> &Path {
        &self.audio
    }

    pub fn video(&self) -> &Path {
        &self.video
    }

    pub fn documents(&self) -> &Path {
        &self.documents
    }

    pub fn uploads(&self) -> &Path {
        &self.uploads
    }

    pub fn shipped_files(&self) -> Option<&Path> {
        self.shipped_files.as_deref()
    }
}
