//! Message roles, content parts and cache breakpoints of a canonical request.
//!
//! `cache_control` is Anthropic's prompt-caching breakpoint. It rides on the
//! canonical model so a rebuilt request (system-prompt override, cross-wire
//! translation) re-emits it exactly where the client placed it; wires without
//! prompt caching ignore it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

impl Role {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageDetail {
    Auto,
    Low,
    High,
}

impl ImageDetail {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Low => "low",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheTtl {
    FiveMinutes,
    OneHour,
}

impl CacheTtl {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FiveMinutes => "5m",
            Self::OneHour => "1h",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "5m" => Some(Self::FiveMinutes),
            "1h" => Some(Self::OneHour),
            _ => None,
        }
    }
}

// Why: Anthropic's only cache type is `ephemeral`; the breakpoint is the
// block's presence, the TTL is the one optional knob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheControl {
    pub ttl: Option<CacheTtl>,
}

impl CacheControl {
    pub const EPHEMERAL: Self = Self { ttl: None };

    #[must_use]
    pub const fn with_ttl(ttl: CacheTtl) -> Self {
        Self { ttl: Some(ttl) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemBlock {
    pub text: String,
    pub cache_control: Option<CacheControl>,
}

impl SystemBlock {
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            cache_control: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ImageSource {
    Base64 {
        media_type: String,
        data: String,
        detail: Option<ImageDetail>,
    },
    Url {
        url: String,
        detail: Option<ImageDetail>,
    },
}

#[derive(Debug, Clone)]
pub enum CanonicalContent {
    Text {
        text: String,
        cache_control: Option<CacheControl>,
    },
    Image {
        source: ImageSource,
        cache_control: Option<CacheControl>,
    },
    ToolUse {
        id: String,
        name: String,
        // JSON: MCP tool-call arguments are the tool's own JSON object.
        input: Value,
        // Why: Gemini requires function-call `thoughtSignature` values replayed verbatim.
        signature: Option<String>,
        cache_control: Option<CacheControl>,
    },
    ToolResult {
        tool_use_id: String,
        content: Vec<Self>,
        is_error: bool,
        structured_content: Option<Value>,
        // JSON: MCP `_meta` is an open map of vendor-prefixed keys.
        meta: Option<Value>,
        cache_control: Option<CacheControl>,
    },
    Thinking {
        text: String,
        signature: Option<String>,
        // Why: OpenAI Responses requires the reasoning item ID and encrypted content
        // replayed verbatim for stateless reasoning continuity.
        id: Option<String>,
        encrypted_content: Option<String>,
    },
}

impl CanonicalContent {
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
            cache_control: None,
        }
    }

    #[must_use]
    pub const fn image(source: ImageSource) -> Self {
        Self::Image {
            source,
            cache_control: None,
        }
    }

    // Why: thinking blocks cannot carry a breakpoint on the Messages API.
    #[must_use]
    pub const fn cache_control(&self) -> Option<CacheControl> {
        match self {
            Self::Text { cache_control, .. }
            | Self::Image { cache_control, .. }
            | Self::ToolUse { cache_control, .. }
            | Self::ToolResult { cache_control, .. } => *cache_control,
            Self::Thinking { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CanonicalMessage {
    pub role: Role,
    pub content: Vec<CanonicalContent>,
}
