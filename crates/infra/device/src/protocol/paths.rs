//! The device's names for the routes it calls beside the bridge routes, each
//! the `systemprompt_models::ApiPaths` constant of the same route, relative to
//! the instance base URL or its origin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::ApiPaths;

pub const FALLBACK_DEVICE_LINK: &str = ApiPaths::BRIDGE_DEVICE_LINK_FALLBACK;
pub const MODELS: &str = ApiPaths::GATEWAY_MODELS;
pub const HOOKS_GOVERN: &str = ApiPaths::PUBLIC_HOOKS_GOVERN;
pub const CONSUMER_RESOURCES: &str = ApiPaths::CONSUMER_RESOURCES;
pub const INSTANCE_MCP_PREFIX: &str = ApiPaths::MCP_SERVER_PREFIX;
pub const BRAIN_FACTS: &str = ApiPaths::BRAIN_FACTS;
pub const BRAIN_SUBJECTS: &str = ApiPaths::BRAIN_SUBJECTS;
pub const BRAIN_DISPUTES: &str = ApiPaths::BRAIN_DISPUTES;
pub const BRAIN_PROPOSALS: &str = ApiPaths::BRAIN_PROPOSALS;
