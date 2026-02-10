use std::path::{Path, PathBuf};

pub trait AssetPaths: Send + Sync {
    fn storage_files(&self) -> &Path;
    fn web_dist(&self) -> &Path;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetType {
    Css,
    Html,
    Image,
    Font,
    JavaScript,
}

#[derive(Debug, Clone)]
pub struct AssetDefinition {
    source: PathBuf,
    destination: &'static str,
    asset_type: AssetType,
    required: bool,
}

#[derive(Debug)]
pub struct AssetDefinitionBuilder {
    source: PathBuf,
    destination: &'static str,
    asset_type: AssetType,
    required: bool,
}

impl AssetDefinitionBuilder {
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

    #[must_use]
    pub const fn optional(mut self) -> Self {
        self.required = false;
        self
    }

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
    pub fn builder(
        source: impl Into<PathBuf>,
        destination: &'static str,
        asset_type: AssetType,
    ) -> AssetDefinitionBuilder {
        AssetDefinitionBuilder::new(source, destination, asset_type)
    }

    #[must_use]
    pub fn css(source: impl Into<PathBuf>, destination: &'static str) -> Self {
        Self::builder(source, destination, AssetType::Css).build()
    }

    #[must_use]
    pub fn html(source: impl Into<PathBuf>, destination: &'static str) -> Self {
        Self::builder(source, destination, AssetType::Html).build()
    }

    #[must_use]
    pub fn image(source: impl Into<PathBuf>, destination: &'static str) -> Self {
        Self::builder(source, destination, AssetType::Image).build()
    }

    #[must_use]
    pub fn font(source: impl Into<PathBuf>, destination: &'static str) -> Self {
        Self::builder(source, destination, AssetType::Font).build()
    }

    #[must_use]
    pub fn javascript(source: impl Into<PathBuf>, destination: &'static str) -> Self {
        Self::builder(source, destination, AssetType::JavaScript).build()
    }

    #[must_use]
    pub fn js(source: impl Into<PathBuf>, destination: &'static str) -> Self {
        Self::javascript(source, destination)
    }

    #[must_use]
    pub fn source(&self) -> &Path {
        &self.source
    }

    #[must_use]
    pub const fn destination(&self) -> &str {
        self.destination
    }

    #[must_use]
    pub const fn asset_type(&self) -> AssetType {
        self.asset_type
    }

    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }
}
