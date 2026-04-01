//! Tests for session analytics header extraction, user agent parsing, and device detection.

use axum::http::{HeaderMap, HeaderValue};

fn create_headers_with_user_agent(ua: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_str(ua).unwrap());
    headers
}

fn create_headers_with_ip(ip: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_str(ip).unwrap());
    headers
}

fn create_full_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "user-agent",
        HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0"),
    );
    headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.1.1"));
    headers.insert("x-fingerprint", HeaderValue::from_static("fp_abc123"));
    headers.insert("accept-language", HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert(
        "referer",
        HeaderValue::from_static("https://google.com/search?q=test"),
    );
    headers
}

mod header_extraction;
mod browser_os_detection;
mod bot_and_misc;
