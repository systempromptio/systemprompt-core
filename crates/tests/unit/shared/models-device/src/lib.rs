//! Unit tests for the wire bodies a device exchanges with an instance, in
//! systemprompt-models: the bridge session, catalog, heartbeat, envelope and
//! discovery bodies, the document client bodies, the brain client bodies and
//! the notification feed.
//!
//! - Source: `crates/shared/models/src/bridge/catalog.rs`
//! - Test: `crates/tests/unit/shared/models-device/src/bridge_catalog.rs`

#[cfg(test)]
mod brain_client;
#[cfg(test)]
mod bridge_catalog;
#[cfg(test)]
mod bridge_envelope;
#[cfg(test)]
mod bridge_heartbeat;
#[cfg(test)]
mod bridge_instance_discovery;
#[cfg(test)]
mod documents_client;
#[cfg(test)]
mod documents_client_feed;
#[cfg(test)]
mod notifications_feed;
