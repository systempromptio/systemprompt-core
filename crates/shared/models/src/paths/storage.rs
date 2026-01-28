use std::path::{Path, PathBuf};

use super::PathError;
use crate::profile::PathsConfig;

#[derive(Debug, Clone)]
pub struct StoragePaths {
    root: PathBuf,
    files: PathBuf,
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
}

impl StoragePaths {
    pub fn from_profile(paths: &PathsConfig) -> Result<Self, PathError> {
        let root = paths
            .storage
            .as_ref()
            .ok_or(PathError::NotConfigured { field: "storage" })?;

        let root = PathBuf::from(root);
        let files = root.join("files");

        Ok(Self {
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
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn files(&self) -> &Path {
        &self.files
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
}
