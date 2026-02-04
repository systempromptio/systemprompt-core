use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CspPolicy {
    pub default_src: Vec<String>,
    pub script_src: Vec<String>,
    pub style_src: Vec<String>,
    pub img_src: Vec<String>,
    pub connect_src: Vec<String>,
    pub font_src: Vec<String>,
    pub frame_src: Vec<String>,
    pub base_uri: Vec<String>,
}

impl CspPolicy {
    pub fn strict() -> Self {
        Self {
            default_src: vec!["'self'".to_string()],
            script_src: vec!["'self'".to_string(), "'unsafe-inline'".to_string()],
            style_src: vec!["'self'".to_string(), "'unsafe-inline'".to_string()],
            img_src: vec!["'self'".to_string(), "data:".to_string()],
            connect_src: vec!["'self'".to_string()],
            font_src: vec!["'self'".to_string()],
            frame_src: vec!["'none'".to_string()],
            base_uri: vec!["'self'".to_string()],
        }
    }

    pub fn with_cdn(cdn_origins: &[&str]) -> Self {
        let mut policy = Self::strict();
        for origin in cdn_origins {
            policy.script_src.push((*origin).to_string());
            policy.style_src.push((*origin).to_string());
        }
        policy
    }

    pub fn to_header_value(&self) -> String {
        let mut directives = Vec::new();

        if !self.default_src.is_empty() {
            directives.push(format!("default-src {}", self.default_src.join(" ")));
        }
        if !self.script_src.is_empty() {
            directives.push(format!("script-src {}", self.script_src.join(" ")));
        }
        if !self.style_src.is_empty() {
            directives.push(format!("style-src {}", self.style_src.join(" ")));
        }
        if !self.img_src.is_empty() {
            directives.push(format!("img-src {}", self.img_src.join(" ")));
        }
        if !self.connect_src.is_empty() {
            directives.push(format!("connect-src {}", self.connect_src.join(" ")));
        }
        if !self.font_src.is_empty() {
            directives.push(format!("font-src {}", self.font_src.join(" ")));
        }
        if !self.frame_src.is_empty() {
            directives.push(format!("frame-src {}", self.frame_src.join(" ")));
        }
        if !self.base_uri.is_empty() {
            directives.push(format!("base-uri {}", self.base_uri.join(" ")));
        }

        directives.join("; ")
    }

    pub fn to_mcp_domains(&self) -> systemprompt_models::mcp::McpCspDomains {
        systemprompt_models::mcp::McpCspDomains {
            connect_domains: Self::extract_domains(&self.connect_src),
            resource_domains: self.extract_resource_domains(),
            frame_domains: Self::extract_domains(&self.frame_src),
            base_uri_domains: Self::extract_domains(&self.base_uri),
        }
    }

    fn extract_domains(sources: &[String]) -> Vec<String> {
        sources
            .iter()
            .filter(|s| !s.starts_with('\'') && *s != "data:")
            .cloned()
            .collect()
    }

    fn extract_resource_domains(&self) -> Vec<String> {
        let all_sources = self
            .script_src
            .iter()
            .chain(self.style_src.iter())
            .chain(self.img_src.iter())
            .chain(self.font_src.iter());

        let mut domains: Vec<String> = all_sources
            .filter(|s| !s.starts_with('\'') && *s != "data:")
            .cloned()
            .collect();

        domains.sort();
        domains.dedup();
        domains
    }
}

#[derive(Debug, Default)]
pub struct CspBuilder {
    policy: CspPolicy,
}

impl CspBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn strict() -> Self {
        Self {
            policy: CspPolicy::strict(),
        }
    }

    pub fn default_src(mut self, sources: Vec<String>) -> Self {
        self.policy.default_src = sources;
        self
    }

    pub fn script_src(mut self, sources: Vec<String>) -> Self {
        self.policy.script_src = sources;
        self
    }

    pub fn add_script_src(mut self, source: &str) -> Self {
        self.policy.script_src.push(source.to_string());
        self
    }

    pub fn style_src(mut self, sources: Vec<String>) -> Self {
        self.policy.style_src = sources;
        self
    }

    pub fn add_style_src(mut self, source: &str) -> Self {
        self.policy.style_src.push(source.to_string());
        self
    }

    pub fn img_src(mut self, sources: Vec<String>) -> Self {
        self.policy.img_src = sources;
        self
    }

    pub fn connect_src(mut self, sources: Vec<String>) -> Self {
        self.policy.connect_src = sources;
        self
    }

    pub fn add_connect_src(mut self, source: &str) -> Self {
        self.policy.connect_src.push(source.to_string());
        self
    }

    pub fn font_src(mut self, sources: Vec<String>) -> Self {
        self.policy.font_src = sources;
        self
    }

    pub fn frame_src(mut self, sources: Vec<String>) -> Self {
        self.policy.frame_src = sources;
        self
    }

    pub fn base_uri(mut self, sources: Vec<String>) -> Self {
        self.policy.base_uri = sources;
        self
    }

    pub fn build(self) -> CspPolicy {
        self.policy
    }
}
