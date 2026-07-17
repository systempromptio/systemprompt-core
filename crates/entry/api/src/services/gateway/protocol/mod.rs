//! Protocol translation between caller and upstream LLM wire formats.
//!
//! The [`canonical`] model is the hub: [`inbound`] adapters parse caller
//! requests into it and render responses back out, while [`outbound`] adapters
//! send it to upstream providers and convert their replies and streams into
//! [`canonical_response`] events. This indirection lets any supported inbound
//! protocol target any supported upstream provider.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod canonical;
pub mod canonical_response;
pub mod inbound;
pub mod outbound;

pub use canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, CanonicalToolChoice,
    ImageSource, Role, ThinkingConfig,
};
pub use canonical_response::{
    CanonicalEvent, CanonicalResponse, CanonicalStopReason, CanonicalUsage, ContentBlockKind,
};
pub use inbound::{InboundAdapter, InboundParseError, anthropic_messages, openai_responses};
pub use outbound::{
    OutboundAdapter, OutboundAdapterRegistration, OutboundCtx, OutboundOutcome,
    anthropic as outbound_anthropic, openai_chat, openai_responses as outbound_openai_responses,
};
