//! Gateway and credential-bound storage for minted tokens.

use crate::config;
use crate::gateway::types::HelperOutput;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};
use systemprompt_identifiers::ValidatedUrl;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialBinding {
    gateway: String,
    digest: String,
}

impl CredentialBinding {
    pub fn capture(cfg: &config::Config) -> io::Result<Self> {
        let gateway = config::trust::GatewayIdentity::new(&config::gateway_url_or_default(cfg))
            .map_err(io::Error::other)?
            .as_str()
            .to_owned();
        let mut sources = Vec::new();
        for suffix in ["DEVICE_CERT", "DEVICE_CERT_LABEL", "DEVICE_CERT_SHA256"] {
            match std::env::var(crate::brand::brand().env(suffix)) {
                Ok(value) if value.trim().is_empty() => {
                    return Err(io::Error::other(format!("configured {suffix} is empty")));
                },
                Ok(_) | Err(std::env::VarError::NotPresent) => {},
                Err(e) => return Err(io::Error::other(format!("{suffix}: {e}"))),
            }
        }
        if let Some(pat) = super::providers::pat::read_source(cfg)? {
            sources.push(("pat", crate::hash::sha256_hex(pat.as_str().as_bytes())));
        }
        if let Some(session) = &cfg.session
            && session.enabled.unwrap_or(false)
        {
            let generation = session.generation.ok_or_else(|| {
                io::Error::other("legacy session has no credential identity; sign in again")
            })?;
            sources.push(("session", generation.to_string()));
        }
        if cfg.cert_keystore_ref().is_some() || super::device_cert_env_configured() {
            let cert = super::keystore::platform_source(
                cfg.cert_keystore_ref().map(crate::ids::KeystoreRef::as_str),
            )
            .load()
            .map_err(io::Error::other)?;
            sources.push(("mtls", cert.fingerprint.as_str().to_owned()));
        }
        if sources.is_empty() {
            return Err(io::Error::other("no credential identity configured"));
        }
        Ok(Self {
            gateway,
            digest: crate::hash::sha256_hex(&serde_json::to_vec(&sources)?),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    output: HelperOutput,
    expires_at: u64,
    gateway: ValidatedUrl,
    #[serde(default)]
    binding: Option<CredentialBinding>,
}

fn cache_path() -> io::Result<PathBuf> {
    let base = crate::basedirs::cache_dir()
        .ok_or_else(|| io::Error::other("credential cache path unresolvable"))?;
    Ok(base
        .join(crate::brand::brand().working_dir_name)
        .join("cache.json"))
}

pub fn read_valid(gateway: &ValidatedUrl) -> io::Result<Option<HelperOutput>> {
    read_with_threshold(gateway, 30)
}

pub fn read_with_threshold(
    gateway: &ValidatedUrl,
    min_remaining_secs: u64,
) -> io::Result<Option<HelperOutput>> {
    let cfg = config::load().map_err(io::Error::other)?;
    read_for(&cfg, gateway, min_remaining_secs)
}

pub fn read_for(
    cfg: &config::Config,
    gateway: &ValidatedUrl,
    min_remaining_secs: u64,
) -> io::Result<Option<HelperOutput>> {
    let Some(entry) = read_entry()? else {
        return Ok(None);
    };
    let binding = CredentialBinding::capture(cfg)?;
    if &entry.gateway != gateway || entry.binding.as_ref() != Some(&binding) {
        clear()?;
        return Ok(None);
    }
    let now = now()?;
    Ok(is_still_valid(entry.expires_at, now, min_remaining_secs).then_some(entry.output))
}

pub fn cached_gateway() -> io::Result<Option<ValidatedUrl>> {
    Ok(read_entry()?.map(|entry| entry.gateway))
}

fn read_entry() -> io::Result<Option<CacheEntry>> {
    let path = cache_path()?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(io::Error::new(
                e.kind(),
                format!("read {}: {e}", path.display()),
            ));
        },
    };
    serde_json::from_slice(&bytes).map(Some).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("parse {}: {e}; sign in again to replace it", path.display()),
        )
    })
}

#[must_use]
pub const fn is_still_valid(expires_at: u64, now: u64, min_remaining_secs: u64) -> bool {
    expires_at > now.saturating_add(min_remaining_secs)
}

pub fn clear() -> io::Result<()> {
    let path = cache_path()?;
    crate::fsutil::remove_verified(&path)
}

fn now() -> io::Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs())
}

pub fn write_bound(
    gateway: &ValidatedUrl,
    output: &HelperOutput,
    binding: &CredentialBinding,
) -> io::Result<()> {
    let current = CredentialBinding::capture(&config::load().map_err(io::Error::other)?)?;
    let origin = config::trust::GatewayIdentity::new(gateway).map_err(io::Error::other)?;
    if &current != binding || origin.as_str() != binding.gateway {
        return Err(io::Error::other(
            "gateway or credentials changed during authentication; retry sign-in",
        ));
    }
    let entry = CacheEntry {
        output: output.clone(),
        expires_at: now()?.saturating_add(output.ttl),
        gateway: gateway.clone(),
        binding: Some(binding.clone()),
    };
    crate::fsutil::atomic_write_0600(&cache_path()?, &serde_json::to_vec(&entry)?)
}
