//! Asset declaration value types used by extensions that ship CSS, HTML
//! fragments, fonts, images, or JavaScript that the host must copy into the
//! web distribution.

use std::path::{Path, PathBuf};

/// Resolves the on-disk locations the host writes assets to.
pub trait AssetPaths: Send + Sync {
    /// Returns the storage-files directory (typically `<storage>/files`).
    fn storage_files(&self) -> &Path;
    /// Returns the web distribution root.
    fn web_dist(&self) -> &Path;
}

/// Coarse classification of a static asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetType {
    /// CSS stylesheet.
    Css,
    /// HTML fragment.
    Html,
    /// Image file (any format).
    Image,
    /// Font file.
    Font,
    /// JavaScript source.
    JavaScript,
}

/// Static asset to be copied into the web distribution.
#[derive(Debug, Clone)]
pub struct AssetDefinition {
    source: PathBuf,
    destination: &'static str,
    asset_type: AssetType,
    required: bool,
}

/// Builder for [`AssetDefinition`] values.
#[derive(Debug)]
pub struct AssetDefinitionBuilder {
    source: PathBuf,
    destination: &'static str,
    asset_type: AssetType,
    required: bool,
}

impl AssetDefinitionBuilder {
    /// Constructs a builder seeded with the source path, destination
    /// location, and asset type. Required-by-default.
    pub fn new(
        source: impl Into<PathBuf>,
        destination: &'static str,
        asset_type: AssetType,
    ) -> Self {
        Self {
            source: source.into(),
            destination,
            asset_type,
            required: true,
        }
    }

    /// Marks the asset as optional (a missing source file is not a load
    /// error).
    #[must_use]
    pub const fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    /// Finalises the builder into an [`AssetDefinition`].
    #[must_use]
    pub fn build(self) -> AssetDefinition {
        AssetDefinition {
            source: self.source,
            destination: self.destination,
            asset_type: self.asset_type,
            required: self.required,
        }
    }
}

impl AssetDefinition {
    /// Returns a fresh [`AssetDefinitionBuilder`].
    pub fn builder(
        source: impl Into<PathBuf>,
        destination: &'static str,
        asset_type: AssetType,
    ) -> AssetDefinitionBuilder {
        AssetDefinitionBuilder::new(source, destination, asset_type)
    }

    /// Convenience constructor for a required CSS asset.
    #[must_use]
    pub fn css(source: impl Into<PathBuf>, destination: &'static str) -> Self {
        Self::builder(source, destination, AssetType::Css).build()
    }

    /// Convenience constructor for a required HTML asset.
    #[must_use]
    pub fn html(source: impl Into<PathBuf>, destination: &'static str) -> Self {
        Self::builder(source, destination, AssetType::Html).build()
    }

    /// Convenience constructor for a required image asset.
    #[must_use]
    pub fn image(source: impl Into<PathBuf>, destination: &'static str) -> Self {
        Self::builder(source, destination, AssetType::Image).build()
    }

    /// Convenience constructor for a required font asset.
    #[must_use]
    pub fn font(source: impl Into<PathBuf>, destination: &'static str) -> Self {
        Self::builder(source, destination, AssetType::Font).build()
    }

    /// Convenience constructor for a required JavaScript asset.
    #[must_use]
    pub fn javascript(source: impl Into<PathBuf>, destination: &'static str) -> Self {
        Self::builder(source, destination, AssetType::JavaScript).build()
    }

    /// Alias for [`Self::javascript`].
    #[must_use]
    pub fn js(source: impl Into<PathBuf>, destination: &'static str) -> Self {
        Self::javascript(source, destination)
    }

    /// Returns the source path of this asset on the extension's filesystem.
    #[must_use]
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Returns the destination path (relative to the web distribution
    /// root).
    #[must_use]
    pub const fn destination(&self) -> &str {
        self.destination
    }

    /// Returns the asset's classification.
    #[must_use]
    pub const fn asset_type(&self) -> AssetType {
        self.asset_type
    }

    /// Returns true if missing this asset must abort startup.
    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }
}
