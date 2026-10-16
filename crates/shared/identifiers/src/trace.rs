//! Distributed-tracing identifier with a `system` constant.
//!
//! A [`TraceId`] is whatever the caller sent in `x-trace-id`, else the trace id
//! of an inbound W3C `traceparent`, else a generated UUID. [`TraceId::w3c_hex`]
//! is the 32-hex form an outbound `traceparent` needs: the id itself when it is
//! already W3C, the UUID's hex digits when it is a UUID, and `None` for any
//! other shape (such as `system`), which is then not propagated.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(TraceId, generate, schema);

const W3C_TRACE_ID_LEN: usize = 32;
const W3C_SPAN_ID_LEN: usize = 16;

impl TraceId {
    pub fn system() -> Self {
        Self("system".to_owned())
    }

    #[must_use]
    pub fn from_traceparent(header: &str) -> Option<Self> {
        let mut parts = header.trim().split('-');
        let version = parts.next()?;
        let trace_id = parts.next()?;
        let parent_id = parts.next()?;
        let flags = parts.next()?;
        if version.len() != 2 || !is_lower_hex(version) || version == "ff" {
            return None;
        }
        if version == "00" && parts.next().is_some() {
            return None;
        }
        if trace_id.len() != W3C_TRACE_ID_LEN || !is_lower_hex(trace_id) || is_all_zero(trace_id) {
            return None;
        }
        if parent_id.len() != W3C_SPAN_ID_LEN || !is_lower_hex(parent_id) || is_all_zero(parent_id)
        {
            return None;
        }
        if flags.len() != 2 || !is_lower_hex(flags) {
            return None;
        }
        Some(Self(trace_id.to_owned()))
    }

    #[must_use]
    pub fn w3c_hex(&self) -> Option<String> {
        if self.0.len() == W3C_TRACE_ID_LEN && is_lower_hex(&self.0) && !is_all_zero(&self.0) {
            return Some(self.0.clone());
        }
        let uuid = uuid::Uuid::parse_str(&self.0).ok()?;
        (!uuid.is_nil()).then(|| uuid.simple().to_string())
    }

    #[must_use]
    pub fn traceparent(&self) -> Option<String> {
        let trace_id = self.w3c_hex()?;
        let span = uuid::Uuid::new_v4().simple().to_string();
        let span_id = &span[..W3C_SPAN_ID_LEN];
        Some(format!("00-{trace_id}-{span_id}-01"))
    }
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_all_zero(value: &str) -> bool {
    value.bytes().all(|b| b == b'0')
}
