//! Anthropic Messages API wire types.
//!
//! Re-exports the request, response, web-search, and streaming-event structs
//! that mirror Anthropic's JSON shape. The model catalogue (ids, pricing,
//! capabilities) lives in the profile `providers` registry, not here.

mod request;
mod response;
mod search;
mod streaming;

pub use request::{
    AnthropicContent, AnthropicContentBlock, AnthropicImageSource, AnthropicMessage,
    AnthropicRequest, AnthropicThinking, AnthropicTool, AnthropicToolChoice,
};
pub use response::{AnthropicResponse, AnthropicUsage};
pub use search::{
    AnthropicCitation, AnthropicSearchContentBlock, AnthropicSearchRequest,
    AnthropicSearchResponse, AnthropicSearchUsage, AnthropicServerTool, AnthropicServerToolUsage,
    AnthropicWebSearchResultItem,
};
pub use streaming::{
    AnthropicContentBlockInfo, AnthropicDelta, AnthropicDeltaUsage, AnthropicMessageDeltaInfo,
    AnthropicMessageInfo, AnthropicStreamError, AnthropicStreamEvent,
};
