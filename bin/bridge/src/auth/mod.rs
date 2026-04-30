pub mod cache;
pub mod keystore;
pub mod loopback;
pub mod providers;
pub mod secret;
pub mod setup;
pub mod types;

use crate::auth::providers::mtls::MtlsProvider;
use crate::auth::providers::pat::PatProvider;
use crate::auth::providers::session::SessionProvider;
use crate::auth::providers::{AuthError, AuthFailedSource, AuthProvider};
use crate::auth::types::HelperOutput;
use crate::config;
use crate::obs::output::diag;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChainError {
    #[error("no credential source succeeded")]
    NoneSucceeded,
    #[error("{provider}: transient failure on preferred provider: {source}")]
    PreferredTransient {
        provider: &'static str,
        #[source]
        source: AuthFailedSource,
    },
}

pub fn acquire_bearer(cfg: &config::Config) -> Result<HelperOutput, ChainError> {
    if let Some(out) = cache::read_valid() {
        return Ok(out);
    }
    run_chain(cfg, true)
}

#[must_use]
pub fn obtain_live_token(cfg: &config::Config) -> Option<HelperOutput> {
    if let Some(out) = cache::read_valid() {
        return Some(out);
    }
    mint_fresh(cfg).ok()
}

#[must_use]
pub fn read_or_refresh(cfg: &config::Config, threshold_secs: u64) -> Option<HelperOutput> {
    if let Some(out) = cache::read_with_threshold(threshold_secs) {
        return Some(out);
    }
    mint_fresh(cfg).ok()
}

#[must_use]
pub fn has_credential_source(cfg: &config::Config) -> bool {
    if std::env::var("SP_BRIDGE_PAT")
        .ok()
        .is_some_and(|s| !s.is_empty())
    {
        return true;
    }
    if let Some(pat) = cfg.pat.as_ref()
        && let Some(file) = pat.file.as_ref() {
            let expanded = expand_home(file);
            if std::path::Path::new(&expanded).exists() {
                return true;
            }
        }
    if let Some(session) = cfg.session.as_ref()
        && session.enabled.unwrap_or(false) {
            return true;
        }
    if let Some(mtls) = cfg.mtls.as_ref()
        && mtls.cert_keystore_ref.is_some() {
            return true;
        }
    false
}

fn expand_home(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    path.to_string()
}

#[must_use]
pub fn provider_chain(cfg: &config::Config) -> Vec<Box<dyn AuthProvider>> {
    vec![
        Box::new(MtlsProvider::new(cfg)),
        Box::new(SessionProvider::new(cfg)),
        Box::new(PatProvider::new(cfg)),
    ]
}

pub fn mint_fresh(cfg: &config::Config) -> Result<HelperOutput, ChainError> {
    run_chain(cfg, true)
}

fn preferred_provider(cfg: &config::Config) -> Option<&'static str> {
    if cfg
        .mtls
        .as_ref()
        .is_some_and(|m| m.cert_keystore_ref.is_some())
    {
        return Some("mtls");
    }
    None
}

fn run_chain(cfg: &config::Config, write_cache: bool) -> Result<HelperOutput, ChainError> {
    let chain = provider_chain(cfg);
    let preferred = preferred_provider(cfg);
    let providers: Vec<&dyn AuthProvider> = chain.iter().map(AsRef::as_ref).collect();
    let result = evaluate_chain(&providers, preferred);
    if write_cache
        && let Ok(out) = result.as_ref()
            && let Err(e) = cache::write(out) {
                diag(&format!("cache write failed (continuing): {e}"));
            }
    result
}

pub fn evaluate_chain(
    chain: &[&dyn AuthProvider],
    preferred: Option<&'static str>,
) -> Result<HelperOutput, ChainError> {
    for p in chain {
        match p.authenticate() {
            Ok(out) => return Ok(out),
            Err(AuthError::NotConfigured) => {},
            Err(AuthError::Failed { provider, source }) => {
                let is_preferred = preferred == Some(provider);
                if is_preferred && !source.is_terminal() {
                    diag(&format!(
                        "{provider}: transient failure on preferred provider: {source}"
                    ));
                    return Err(ChainError::PreferredTransient { provider, source });
                }
                diag(&format!("{provider}: {source}"));
            },
        }
    }
    Err(ChainError::NoneSucceeded)
}
