//! Token and prompt-cache counters carried on an [`AiRequestRecord`].
//!
//! [`AiRequestRecord`]: super::AiRequestRecord
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Copy, Default)]
pub struct TokenInfo {
    pub tokens_used: Option<i32>,
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub reasoning_tokens: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CacheInfo {
    pub hit: bool,
    pub read_tokens: Option<i32>,
    pub creation_tokens: Option<i32>,
}
