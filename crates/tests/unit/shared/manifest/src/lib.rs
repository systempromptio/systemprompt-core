//! Unit tests for the 0.73 catalog in `systemprompt-manifest`: the four
//! registries, their serde shapes, `ResolvedCatalog` validation and
//! resolution, the embedded default catalog, the legacy-shape refusal, the
//! bridge profile and the Vertex rate card.

#[cfg(test)]
mod bridge_profile;
#[cfg(test)]
mod catalog_validation;
#[cfg(test)]
mod credentials;
#[cfg(test)]
mod default_catalog;
#[cfg(test)]
mod deployments_serde;
#[cfg(test)]
mod legacy_refusal;
#[cfg(test)]
mod provider_settings;
#[cfg(test)]
mod resolved_catalog;
#[cfg(test)]
mod resolved_routes;
#[cfg(test)]
mod vertex_rate_card;
