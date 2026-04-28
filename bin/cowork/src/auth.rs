use crate::providers::mtls::MtlsProvider;
use crate::providers::pat::PatProvider;
use crate::providers::session::SessionProvider;
use crate::providers::{AuthError, AuthProvider};
use crate::types::HelperOutput;
use crate::{cache, config};

pub fn obtain_live_token(cfg: &config::Config) -> Option<HelperOutput> {
    if let Some(out) = cache::read_valid() {
        return Some(out);
    }
    mint_fresh(cfg)
}

pub fn read_or_refresh(cfg: &config::Config, threshold_secs: u64) -> Option<HelperOutput> {
    if let Some(out) = cache::read_with_threshold(threshold_secs) {
        return Some(out);
    }
    mint_fresh(cfg)
}

pub fn has_credential_source(cfg: &config::Config) -> bool {
    if std::env::var("SP_COWORK_PAT")
        .ok()
        .filter(|s| !s.is_empty())
        .is_some()
    {
        return true;
    }
    if let Some(pat) = cfg.pat.as_ref() {
        if let Some(file) = pat.file.as_ref() {
            let expanded = expand_home(file);
            if std::path::Path::new(&expanded).exists() {
                return true;
            }
        }
    }
    if let Some(session) = cfg.session.as_ref() {
        if session.enabled.unwrap_or(false) {
            return true;
        }
    }
    if let Some(mtls) = cfg.mtls.as_ref() {
        if mtls
            .cert_keystore_ref
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    }
    path.to_string()
}

pub fn mint_fresh(cfg: &config::Config) -> Option<HelperOutput> {
    let chain: Vec<Box<dyn AuthProvider>> = vec![
        Box::new(MtlsProvider::new(cfg)),
        Box::new(SessionProvider::new(cfg)),
        Box::new(PatProvider::new(cfg)),
    ];
    for p in &chain {
        match p.authenticate() {
            Ok(out) => {
                let _ = cache::write(&out);
                return Some(out);
            },
            Err(AuthError::NotConfigured) => continue,
            Err(AuthError::Failed(_)) => {},
        }
    }
    None
}
