use anyhow::{anyhow, Result};
use handlebars::Handlebars;
use serde_json::Value;
use std::path::Path;
use tokio::fs;

pub async fn load_web_config() -> Result<serde_yaml::Value> {
    let web_config_path = std::env::var("SYSTEMPROMPT_WEB_CONFIG_PATH")
        .ok()
        .or_else(|| {
            systemprompt_models::Config::get()
                .ok()
                .map(|c| c.web_config_path.clone())
        })
        .ok_or_else(|| {
            anyhow!(
                "Web config path not available: set SYSTEMPROMPT_WEB_CONFIG_PATH or initialize \
                 Config"
            )
        })?;

    let content = fs::read_to_string(&web_config_path)
        .await
        .map_err(|e| anyhow!("Failed to read web config at '{}': {}", web_config_path, e))?;

    serde_yaml::from_str(&content).map_err(|e| anyhow!("Failed to parse web config: {}", e))
}

pub fn get_templates_path(config: &serde_yaml::Value) -> Result<String> {
    config
        .get("paths")
        .and_then(|p| p.get("templates"))
        .and_then(|t| t.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("paths.templates must be set in web config"))
}

pub fn get_assets_path(config: &serde_yaml::Value) -> Result<String> {
    config
        .get("paths")
        .and_then(|p| p.get("assets"))
        .and_then(|t| t.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("paths.assets must be set in web config"))
}

#[derive(Debug)]
pub struct TemplateEngine {
    handlebars: Handlebars<'static>,
}

impl TemplateEngine {
    pub async fn new(template_dir: &str) -> Result<Self> {
        let mut handlebars = Handlebars::new();

        let path = Path::new(template_dir);
        if !path.exists() {
            return Err(anyhow!("Template directory not found: {}", template_dir));
        }

        let mut entries = fs::read_dir(path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "html") {
                let Some(file_stem) = path.file_stem() else {
                    continue;
                };
                let template_name = file_stem.to_string_lossy().to_string();

                let content = fs::read_to_string(&path).await?;
                handlebars.register_template_string(&template_name, content)?;
            }
        }

        Ok(Self { handlebars })
    }

    pub fn render(&self, template_name: &str, data: &Value) -> Result<String> {
        self.handlebars
            .render(template_name, data)
            .map_err(|e| anyhow!("Template render failed: {}", e))
    }
}
