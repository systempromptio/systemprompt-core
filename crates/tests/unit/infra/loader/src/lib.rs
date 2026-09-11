//! Unit tests for systemprompt-loader crate

#![allow(clippy::all)]

#[cfg(test)]
mod bundle_bootstrap;
#[cfg(test)]
mod bundle_cache;
#[cfg(test)]
mod bundle_cache_edges;
#[cfg(test)]
mod bundle_compose;
#[cfg(test)]
mod bundle_extract;
#[cfg(test)]
mod bundle_https;
#[cfg(test)]
mod bundle_oci;
#[cfg(test)]
mod bundle_oci_auth;
#[cfg(test)]
mod bundle_oci_push;
#[cfg(test)]
mod bundle_pack_extract;
#[cfg(test)]
mod bundle_profile;
#[cfg(test)]
mod bundle_source_select;
#[cfg(test)]
mod bundle_support;
#[cfg(test)]
mod bundle_verify;
#[cfg(test)]
mod bundle_verify_edges;
#[cfg(test)]
mod config_loader_apps;
#[cfg(test)]
mod config_loader_discovery;
#[cfg(test)]
mod config_loader_errors;
#[cfg(test)]
mod config_loader_gateway;
#[cfg(test)]
mod config_loader_merge;
#[cfg(test)]
mod config_writer;
#[cfg(test)]
mod config_writer_edges;
#[cfg(test)]
mod coverage_gaps;
#[cfg(test)]
mod error_display;
#[cfg(test)]
mod extension_loader;
#[cfg(test)]
mod extension_loader_extra;
#[cfg(test)]
mod extension_registry;
#[cfg(test)]
mod module_loader;
#[cfg(test)]
mod profile_loader;
#[cfg(test)]
mod services_catalog;
#[cfg(test)]
mod services_loader;
#[cfg(test)]
mod services_root_cell;
#[cfg(test)]
mod vertex_discovery_classify;
#[cfg(test)]
mod vertex_discovery_client;
#[cfg(test)]
mod vertex_discovery_merge;
