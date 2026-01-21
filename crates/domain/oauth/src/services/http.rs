use http::HeaderMap;

pub fn is_browser_request(headers: &HeaderMap) -> bool {
    headers
        .get("accept")
        .and_then(|v| {
            v.to_str()
                .map_err(|e| {
                    tracing::debug!(error = %e, "Invalid UTF-8 in Accept header");
                    e
                })
                .ok()
        })
        .is_some_and(|accept| {
            accept.contains("text/html") && !accept.starts_with("application/json")
        })
}
