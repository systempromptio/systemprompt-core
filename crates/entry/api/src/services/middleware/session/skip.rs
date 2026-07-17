//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::modules::ApiPaths;

pub fn should_skip_session_tracking(path: &str) -> bool {
    if path.starts_with(ApiPaths::TRACK_BASE) {
        return false;
    }

    if path.starts_with(ApiPaths::MCP_BASE) {
        return true;
    }

    if path.starts_with(ApiPaths::API_BASE) {
        return true;
    }

    if path.starts_with(ApiPaths::NEXT_BASE) {
        return true;
    }

    if path.starts_with(ApiPaths::STATIC_BASE)
        || path.starts_with(ApiPaths::ASSETS_BASE)
        || path.starts_with(ApiPaths::IMAGES_BASE)
    {
        return true;
    }

    if path == "/health" || path == "/ready" || path == "/healthz" {
        return true;
    }

    if path == "/favicon.ico"
        || path == "/robots.txt"
        || path == "/sitemap.xml"
        || path == "/manifest.json"
    {
        return true;
    }

    if let Some(last_segment) = path.rsplit('/').next()
        && last_segment.contains('.')
    {
        let extension = last_segment.rsplit('.').next().unwrap_or("");
        match extension {
            "html" | "htm" => {},
            _ => return true,
        }
    }

    false
}
