//! What a model call resolves to once its response head arrived with a success
//! status: the head itself, and the stream of events that follows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use crate::llm::stream::EventStream;

/// The status and headers of a model response.
///
/// Header names keep the case the gateway sent; [`ResponseHead::header`]
/// looks one up ignoring case. `Debug` lists the header names only.
#[derive(Clone, PartialEq, Eq)]
pub struct ResponseHead {
    pub status: u16,
    pub headers: Vec<(String, String)>,
}

impl ResponseHead {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

impl fmt::Debug for ResponseHead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("ResponseHead")
            .field("status", &self.status)
            .field("headers", &names)
            .finish()
    }
}

/// A model response that opened: its head and its events.
pub struct ModelResponse {
    pub head: ResponseHead,
    pub events: EventStream,
}

impl fmt::Debug for ModelResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelResponse")
            .field("head", &self.head)
            .finish_non_exhaustive()
    }
}
