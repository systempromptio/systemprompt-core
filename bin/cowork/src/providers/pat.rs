use crate::config::Config;
use crate::http::GatewayClient;
use crate::providers::{AuthError, AuthProvider};
use crate::types::HelperOutput;
use std::{env, fs};

pub struct PatProvider {
    base_url: String,
    pat_source: Option<String>,
}

impl PatProvider {
    pub fn new(config: &Config) -> Self {
        let pat_source = env::var("SP_COWORK_PAT").ok().or_else(|| {
            config
                .pat
                .as_ref()
                .and_then(|p| p.file.as_ref())
                .and_then(|path| fs::read_to_string(expand(path)).ok())
                .map(|s| s.trim().to_string())
        });
        Self {
            base_url: crate::config::gateway_url_or_default(config),
            pat_source,
        }
    }
}

impl AuthProvider for PatProvider {
    fn name(&self) -> &'static str {
        "pat"
    }

    fn authenticate(&self) -> Result<HelperOutput, AuthError> {
        let pat = self.pat_source.as_ref().ok_or(AuthError::NotConfigured)?;
        if pat.is_empty() {
            return Err(AuthError::NotConfigured);
        }
        let client = GatewayClient::new(self.base_url.clone());
        let resp = client.pat_exchange(pat).map_err(AuthError::Failed)?;
        Ok(resp.into())
    }
}

fn expand(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    }
    path.to_string()
}
