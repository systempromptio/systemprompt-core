//! Route paths the device calls that `systemprompt_models::ApiPaths` does not
//! name yet, relative to the instance base URL or its origin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const FALLBACK_DEVICE_LINK: &str = "/bridge-auth/device-link";
pub const MODELS: &str = "/v1/models";
pub const HOOKS_GOVERN: &str = "/api/public/hooks/govern";
pub const CONSUMER_RESOURCES: &str = "/api/v1/consumer/resources";
pub const INSTANCE_MCP_PREFIX: &str = "/api/v1/mcp/";
pub const BRAIN_FACTS: &str = "/api/v1/brain/facts";
pub const BRAIN_SUBJECTS: &str = "/api/v1/brain/subjects";
pub const BRAIN_DISPUTES: &str = "/api/v1/brain/disputes";
pub const BRAIN_PROPOSALS: &str = "/api/v1/brain/proposals";
