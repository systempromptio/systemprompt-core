pub mod anthropic_messages;
pub mod openai_responses;

use bytes::Bytes;
use http::StatusCode;

use super::canonical::CanonicalRequest;
use super::canonical_response::{CanonicalEvent, CanonicalResponse};

#[derive(Debug, thiserror::Error)]
pub enum InboundParseError {
    #[error("invalid request body: {0}")]
    InvalidJson(String),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("unsupported value for {field}: {detail}")]
    Unsupported {
        field: &'static str,
        detail: String,
    },
}

pub trait InboundAdapter: Send + Sync {
    fn wire_name(&self) -> &'static str;
    fn parse_request(&self, raw: &Bytes) -> Result<CanonicalRequest, InboundParseError>;
    fn render_response(&self, response: &CanonicalResponse) -> Bytes;
    fn render_event(&self, event: &CanonicalEvent, model: &str) -> Option<Bytes>;
    fn render_error(&self, status: StatusCode, message: &str) -> Bytes;
    fn streaming_content_type(&self) -> &'static str {
        "text/event-stream"
    }
}
