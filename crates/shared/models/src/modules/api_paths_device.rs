//! [`ApiPaths`] constants for the routes a governed device calls beside the
//! bridge routes: the model routes of the gateway, the verdict hook, the
//! fallback device-link page, the consumer resources and the brain.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ApiPaths;

impl ApiPaths {
    pub const GATEWAY_MODELS: &'static str = "/v1/models";
    pub const GATEWAY_MESSAGES: &'static str = "/v1/messages";
    pub const GATEWAY_CHAT_COMPLETIONS: &'static str = "/v1/chat/completions";

    pub const PUBLIC_HOOKS_GOVERN: &'static str = "/api/public/hooks/govern";

    pub const BRIDGE_DEVICE_LINK_FALLBACK: &'static str = "/bridge-auth/device-link";

    pub const CONSUMER_RESOURCES: &'static str = "/api/v1/consumer/resources";

    pub const MCP_SERVER_PREFIX: &'static str = "/api/v1/mcp/";

    pub const BRAIN_FACTS: &'static str = "/api/v1/brain/facts";
    pub const BRAIN_SUBJECTS: &'static str = "/api/v1/brain/subjects";
    pub const BRAIN_DISPUTES: &'static str = "/api/v1/brain/disputes";
    pub const BRAIN_PROPOSALS: &'static str = "/api/v1/brain/proposals";
}
