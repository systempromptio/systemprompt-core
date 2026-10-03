//! Protocol translation between caller and upstream LLM wire formats.
//!
//! The [`canonical`] model (defined in `systemprompt-wire`) is the hub:
//! [`inbound`] adapters parse caller requests into it and render responses
//! back out, while [`outbound`] adapters send it to upstream providers and
//! convert their replies and streams into canonical events. This indirection
//! lets any supported inbound protocol target any supported upstream provider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod inbound;
pub mod outbound;

pub use systemprompt_wire::canonical;

pub use canonical::{
    CacheControl, CacheTtl, CanonicalContent, CanonicalEvent, CanonicalMessage, CanonicalRequest,
    CanonicalResponse, CanonicalStopReason, CanonicalTool, CanonicalToolChoice, CanonicalUsage,
    CanonicalUsageUpdate, ContentBlockKind, ImageSource, Role, SystemBlock, ThinkingConfig,
};
pub use inbound::{InboundAdapter, InboundParseError, anthropic_messages, openai_responses};
pub use outbound::{
    OutboundAdapter, OutboundAdapterRegistration, OutboundCtx, OutboundOutcome,
    anthropic as outbound_anthropic, openai_chat, openai_responses as outbound_openai_responses,
};
