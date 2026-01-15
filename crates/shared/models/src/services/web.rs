use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebConfig {
    #[serde(default)]
    pub branding: BrandingConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BrandingConfig {
    #[serde(default)]
    pub site_name: Option<String>,

    #[serde(default)]
    pub logo_url: Option<String>,

    #[serde(default)]
    pub primary_color: Option<String>,
}
