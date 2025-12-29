use anyhow::Result;
use systemprompt_models::ContentConfigRaw;

#[derive(Debug, Clone)]
pub struct StaticContentMatcher {
    patterns: Vec<(String, String)>,
}

impl StaticContentMatcher {
    pub fn from_config(config_path: &str) -> Result<Self> {
        let yaml_content = std::fs::read_to_string(config_path)?;
        let config: ContentConfigRaw = serde_yaml::from_str(&yaml_content)?;

        let patterns = config
            .content_sources
            .into_iter()
            .filter(|(_, source)| source.enabled)
            .filter_map(|(source_id, source)| {
                source
                    .sitemap
                    .filter(|s| s.enabled)
                    .map(|sitemap| (sitemap.url_pattern, source_id))
            })
            .collect();

        Ok(Self { patterns })
    }

    pub const fn empty() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    pub fn matches(&self, path: &str) -> Option<(String, String)> {
        self.patterns.iter().find_map(|(pattern, source_id)| {
            extract_slug(path, pattern).map(|slug| (slug, source_id.clone()))
        })
    }
}

fn extract_slug(path: &str, pattern: &str) -> Option<String> {
    let pattern_parts: Vec<&str> = pattern.split('{').collect();
    if pattern_parts.len() != 2 {
        return None;
    }

    let prefix = pattern_parts[0];
    if !path.starts_with(prefix) {
        return None;
    }

    let slug = path.trim_start_matches(prefix).trim_end_matches('/');
    (!slug.is_empty()).then(|| slug.to_string())
}
