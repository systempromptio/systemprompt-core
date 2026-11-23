//! `systemprompt-gateway-entry` — the AI gateway entry surface.
//!
//! The `http` feature carries the gateway's handlers ([`http`]): the
//! inference proxy, the token count and, with `otel`, the telemetry ingest.
//! [`extension::GatewayHttpExtension`] (`gateway-http`) contributes them as a
//! [`GatewaySurface`](systemprompt_gateway::GatewaySurface) the API mount
//! builds and layers, so the API server does not name this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod extension;
#[cfg(feature = "http")]
pub mod http;
