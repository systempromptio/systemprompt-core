//! Route paths, relative to the instance base URL (bridge and core routes) or
//! its origin (consumer routes).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const DISCOVERY: &str = "/.well-known/systemprompt-bridge";
pub const FALLBACK_DEVICE_LINK: &str = "/bridge-auth/device-link";
pub const HEALTH: &str = "/api/v1/health";
pub const CAPABILITIES: &str = "/v1/auth/bridge/capabilities";
pub const SESSION_PAT: &str = "/v1/auth/bridge/session-pat";
pub const SESSION: &str = "/v1/auth/bridge/session";
pub const PAT: &str = "/v1/auth/bridge/pat";
pub const OAUTH_CLIENT: &str = "/v1/auth/bridge/oauth-client";
pub const PUBKEY: &str = "/v1/bridge/pubkey";
pub const MANIFEST: &str = "/v1/bridge/manifest";
pub const PROFILE: &str = "/v1/bridge/profile";
pub const WHOAMI: &str = "/v1/bridge/whoami";
pub const MODELS: &str = "/v1/models";
pub const HEARTBEAT: &str = "/v1/bridge/heartbeat";
pub const DEVICE: &str = "/v1/bridge/device";
pub const PLUGIN_FILES: &str = "/v1/bridge/plugins";
pub const HOOKS_TRACK: &str = "/api/public/hooks/track";
pub const HOOKS_GOVERN: &str = "/api/public/hooks/govern";
pub const CONSUMER_RESOURCES: &str = "/api/v1/consumer/resources";
pub const CONSUMER_RECEIPTS: &str = "/api/v1/consumer/receipts";
pub const CONSUMER_SESSION_BINDINGS: &str = "/api/v1/consumer/session-bindings";
pub const DOCUMENTS: &str = "/api/v1/documents";
pub const DOCUMENT_FOLDERS: &str = "/api/v1/document-folders";
pub const UPLOADS: &str = "/api/v1/uploads";
pub const CONTEXTS: &str = "/api/v1/core/contexts";
pub const OAUTH_TOKEN: &str = "/api/v1/core/oauth/token";
pub const INSTANCE_MCP_PREFIX: &str = "/api/v1/mcp/";
pub const DRIVE_MANIFEST: &str = "/v1/bridge/drive/manifest";
pub const DRIVE_CHANGES: &str = "/v1/bridge/drive/changes";
pub const DRIVE_BLOB: &str = "/v1/bridge/drive/blob";
pub const DRIVE_COMMIT: &str = "/v1/bridge/drive/commit";
pub const DRIVE_TRASH: &str = "/v1/bridge/drive/trash";
pub const DRIVE_MOVE: &str = "/v1/bridge/drive/move";
pub const DRIVE_TOKEN: &str = "/v1/bridge/drive/token";
pub const NOTIFICATIONS: &str = "/v1/bridge/notifications";
pub const NOTIFICATIONS_READ: &str = "/v1/bridge/notifications/read";
pub const NOTIFICATIONS_SEEN: &str = "/v1/bridge/notifications/seen";
pub const BRAIN_FACTS: &str = "/api/v1/brain/facts";
pub const BRAIN_SUBJECTS: &str = "/api/v1/brain/subjects";
pub const BRAIN_DISPUTES: &str = "/api/v1/brain/disputes";
pub const BRAIN_PROPOSALS: &str = "/api/v1/brain/proposals";
