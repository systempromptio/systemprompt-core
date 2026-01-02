pub fn is_valid_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }

    parts.iter().all(|part| part.parse::<u32>().is_ok())
}

pub fn extract_port_from_url(url: &str) -> Option<u16> {
    if let Some(url_after_protocol) = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
    {
        if let Some(host_part) = url_after_protocol.split('/').next() {
            if let Some(port_str) = host_part.split(':').nth(1) {
                return port_str.parse().ok();
            }
        }
        if url.starts_with("https://") {
            Some(443)
        } else {
            Some(80)
        }
    } else {
        None
    }
}

pub async fn list_available_mcp_servers() -> Result<Vec<String>, String> {
    use systemprompt_loader::ConfigLoader;

    ConfigLoader::load()
        .map(|config| config.mcp_servers.keys().cloned().collect())
        .map_err(|e| e.to_string())
}
