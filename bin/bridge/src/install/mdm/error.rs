//! Typed errors for the MDM policy-application surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MdmError {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[error(transparent)]
    DesktopCatalog(#[from] crate::gateway::desktop_catalog::CatalogError),
    #[error("policy partially completed {completed:?}; {source}")]
    Partial {
        completed: super::MdmApplication,
        #[source]
        source: Box<Self>,
    },
    #[error("invalid policy configuration: {key}: {detail}")]
    InvalidConfig { key: &'static str, detail: String },
    #[error("invalid policy configuration: {key}: {source}")]
    ConfigJson {
        key: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error(transparent)]
    Egress(#[from] super::egress::EgressParseError),
    #[error("gateway url: {0}")]
    GatewayUrl(#[from] url::ParseError),
    #[error("{path}: expected mode 0700")]
    HelperMode { path: PathBuf },
    #[error("forced login method conflicts with the managed settings: {0}")]
    ForcedLoginConflict(String),
    #[error("{what} remains after policy removal: {detail}")]
    RemovalIncomplete { what: &'static str, detail: String },
    #[error("USER cannot identify a managed-preferences directory: {detail}")]
    ManagedPrefsUser { detail: String },
    #[error("USER cannot identify a managed-preferences directory: {0}")]
    ManagedPrefsUserUnset(#[source] std::env::VarError),
    #[error(transparent)]
    Config(#[from] crate::config::ConfigReadError),
    #[error(transparent)]
    Trust(#[from] crate::config::TrustError),
    #[error(transparent)]
    Store(#[from] crate::config::store::ConfigStoreError),
    #[error("{action} {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot resolve {0}")]
    Resolve(&'static str),
    #[error("{path} is not valid JSON: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{path}: \"env\" is present but is not an object")]
    EnvNotObject { path: PathBuf },
    #[error(
        "gateway url {gateway} uses http:// for a non-loopback host; Bridge rejects this. Use \
         https:// or http://127.0.0.1:<port>."
    )]
    InsecureGateway { gateway: String },
    #[error("--apply on {os} must be run from a {os} binary")]
    WrongHostOs { os: &'static str },
    #[cfg(target_os = "macos")]
    #[error(transparent)]
    Elevation(#[from] crate::install::elevate::ElevationError),
    #[cfg(target_os = "windows")]
    #[error("org-plugins {context}: {source}")]
    OrgPlugins {
        context: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    #[cfg(target_os = "windows")]
    #[error(
        "{} is not provisioned; run install --apply as Administrator before using Cowork",
        .path.display()
    )]
    OrgPluginsMissing { path: PathBuf },
    #[cfg(target_os = "windows")]
    #[error("policy writer: {0}")]
    PolicyWriter(#[source] Box<crate::install::policy_writer::PolicyWriterError>),
}
