use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentTypeListOutput {
    pub content_types: Vec<ContentTypeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentTypeSummary {
    pub name: String,
    pub source_id: String,
    pub category_id: String,
    pub enabled: bool,
    pub path: String,
    pub url_pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentTypeDetailOutput {
    pub name: String,
    pub source_id: String,
    pub category_id: String,
    pub enabled: bool,
    pub path: String,
    pub description: String,
    pub allowed_content_types: Vec<String>,
    pub sitemap: Option<SitemapInfo>,
    pub branding: Option<BrandingInfo>,
    pub indexing: Option<IndexingInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SitemapInfo {
    pub enabled: bool,
    pub url_pattern: String,
    pub priority: f32,
    pub changefreq: String,
    pub fetch_from: String,
    pub parent_route: Option<ParentRouteInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ParentRouteInfo {
    pub enabled: bool,
    pub url: String,
    pub priority: f32,
    pub changefreq: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BrandingInfo {
    pub name: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub keywords: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct IndexingInfo {
    pub clear_before: bool,
    pub recursive: bool,
    pub override_existing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentTypeCreateOutput {
    pub name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentTypeEditOutput {
    pub name: String,
    pub message: String,
    pub changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContentTypeDeleteOutput {
    pub deleted: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateListOutput {
    pub templates: Vec<TemplateSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateSummary {
    pub name: String,
    pub content_types: Vec<String>,
    pub file_exists: bool,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateDetailOutput {
    pub name: String,
    pub content_types: Vec<String>,
    pub file_path: String,
    pub file_exists: bool,
    pub variables: Vec<String>,
    pub preview_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateCreateOutput {
    pub name: String,
    pub file_path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateEditOutput {
    pub name: String,
    pub message: String,
    pub changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TemplateDeleteOutput {
    pub deleted: String,
    pub file_deleted: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssetListOutput {
    pub assets: Vec<AssetSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssetSummary {
    pub path: String,
    pub asset_type: AssetType,
    pub size_bytes: u64,
    pub modified: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AssetType {
    Css,
    Logo,
    Favicon,
    Font,
    Image,
    Other,
}

impl std::fmt::Display for AssetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Css => write!(f, "css"),
            Self::Logo => write!(f, "logo"),
            Self::Favicon => write!(f, "favicon"),
            Self::Font => write!(f, "font"),
            Self::Image => write!(f, "image"),
            Self::Other => write!(f, "other"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssetDetailOutput {
    pub path: String,
    pub absolute_path: String,
    pub asset_type: AssetType,
    pub size_bytes: u64,
    pub modified: String,
    pub referenced_in: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SitemapShowOutput {
    pub routes: Vec<SitemapRoute>,
    pub total_routes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SitemapRoute {
    pub url: String,
    pub priority: f32,
    pub changefreq: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SitemapGenerateOutput {
    pub output_path: String,
    pub routes_count: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationOutput {
    pub valid: bool,
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationIssue {
    pub category: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatesConfig {
    pub templates: std::collections::HashMap<String, TemplateEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateEntry {
    pub content_types: Vec<String>,
}
