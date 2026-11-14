//! `systemprompt-mcp-host` — the least-privilege child hosts of the API.
//!
//! A managed MCP server and a spawned agent are children of the API that boot
//! from the environment the API spawned them with (profile path, database
//! URLs, the public verification key set and the secrets their manifest
//! grants) and nothing else:
//!
//! - [`HostContext`] is that context. It is composed by
//!   [`HostKernel`](systemprompt_runtime::HostKernel), the runtime's own
//!   composition stages, so the authz hook, subject providers and extension
//!   state a host enforces are the API's by construction.
//! - [`toolsets`] serves a YAML-declared MCP server: the deployment's
//!   `toolsets:` built from the factories registered with
//!   [`submit_toolset!`], behind the hop-verifying MCP router. A server that
//!   names no `binary:` is this product binary re-run as
//!   `admin mcp serve --server <id>`.
//! - [`AgentHost`] composes an agent's collaborators on a [`HostContext`].
//!
//! # What a toolset factory can reach
//!
//! A factory receives the [`HostContext`] and its server id. It can reach the
//! database pool, the services view, the granted secrets (only those the
//! manifest grants), the verify-only key ring, the authz hook and subject
//! providers, the artifact ingest, the MCP session repository, the user
//! repository, the document store, notifications, the extension state and a
//! [`LinkIssuer`](systemprompt_runtime::documents::links::LinkIssuer) for what
//! only the API may sign. The per-user subject of a call is not a factory
//! input: it arrives with each call as the hop-verified `RequestContext` in
//! [`ToolsetCall`](systemprompt_mcp::ToolsetCall), so a toolset acting for a
//! user (an agent-runs toolset, for one) reads it there. A factory cannot
//! reach a signing key, the OAuth repositories, the AI repositories, the
//! profile's secrets source or any secret the manifest did not grant.
//!
//! # Feature flags
//!
//! | Feature         | Effect                                                     |
//! |-----------------|------------------------------------------------------------|
//! | (default)       | Host context, toolset host, agent host                     |
//! | `notifications` | Builds the runtime with the notifications service          |
//! | `brain`         | Builds the runtime with the brain (implies notifications)  |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agent_host;
pub mod host;
pub mod toolsets;

pub use agent_host::{AgentHost, AgentHostError};
pub use host::{HostBootError, HostContext, PROFILE_ENV};

#[doc(hidden)]
pub use systemprompt_mcp::__inventory as __toolset_inventory;
#[doc(hidden)]
pub use systemprompt_runtime::RegisteredToolsetId as __RegisteredToolsetId;
