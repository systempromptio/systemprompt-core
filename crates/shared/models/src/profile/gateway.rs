use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub routes: Vec<GatewayRoute>,
}

impl GatewayConfig {
    pub fn find_route(&self, model: &str) -> Option<&GatewayRoute> {
        self.routes.iter().find(|route| route.matches(model))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRoute {
    pub model_pattern: String,
    pub provider: GatewayProvider,
    pub endpoint: String,
    pub api_key_secret: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
}

impl GatewayRoute {
    pub fn matches(&self, model: &str) -> bool {
        match_pattern(&self.model_pattern, model)
    }

    pub fn effective_upstream_model<'a>(&'a self, requested: &'a str) -> &'a str {
        self.upstream_model.as_deref().unwrap_or(requested)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GatewayProvider {
    Anthropic,
    #[serde(rename = "openai")]
    OpenAI,
    Gemini,
    Moonshot,
    Qwen,
}

impl GatewayProvider {
    pub const fn is_openai_compatible(self) -> bool {
        matches!(self, Self::OpenAI | Self::Moonshot | Self::Qwen)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAI => "openai",
            Self::Gemini => "gemini",
            Self::Moonshot => "moonshot",
            Self::Qwen => "qwen",
        }
    }
}

fn match_pattern(pattern: &str, model: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return model.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return model.ends_with(suffix);
    }
    pattern == model
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_pattern_matches() {
        assert!(match_pattern("claude-sonnet-4-6", "claude-sonnet-4-6"));
        assert!(!match_pattern("claude-sonnet-4-6", "claude-opus-4-7"));
    }

    #[test]
    fn prefix_wildcard_matches() {
        assert!(match_pattern("claude-*", "claude-sonnet-4-6"));
        assert!(!match_pattern("claude-*", "moonshot-v1-8k"));
    }

    #[test]
    fn catch_all_matches() {
        assert!(match_pattern("*", "any-model-name"));
    }

    #[test]
    fn route_finds_matching_model() {
        let config = GatewayConfig {
            enabled: true,
            routes: vec![GatewayRoute {
                model_pattern: "kimi-*".to_string(),
                provider: GatewayProvider::Moonshot,
                endpoint: "https://api.moonshot.ai/v1".to_string(),
                api_key_secret: "moonshot".to_string(),
                upstream_model: Some("moonshot-v1-32k".to_string()),
                extra_headers: HashMap::new(),
            }],
        };
        let route = config.find_route("kimi-latest");
        assert!(route.is_some(), "route must match");
        let route = route.unwrap_or_else(|| unreachable!());
        assert_eq!(route.provider, GatewayProvider::Moonshot);
        assert_eq!(
            route.effective_upstream_model("kimi-latest"),
            "moonshot-v1-32k"
        );
    }

    #[test]
    fn provider_openai_compatibility() {
        assert!(GatewayProvider::OpenAI.is_openai_compatible());
        assert!(GatewayProvider::Moonshot.is_openai_compatible());
        assert!(GatewayProvider::Qwen.is_openai_compatible());
        assert!(!GatewayProvider::Anthropic.is_openai_compatible());
        assert!(!GatewayProvider::Gemini.is_openai_compatible());
    }
}
