pub fn parse_user_agent(ua: &str) -> (Option<String>, Option<String>, Option<String>) {
    let ua_lower = ua.to_lowercase();

    let device_type = parse_device_type(&ua_lower);
    let browser = parse_browser(&ua_lower);
    let os = parse_os(&ua_lower);

    (device_type, browser, os)
}

#[allow(clippy::unnecessary_wraps)]
fn parse_device_type(ua_lower: &str) -> Option<String> {
    if ua_lower.contains("mobile") || ua_lower.contains("android") || ua_lower.contains("iphone") {
        Some("mobile".to_string())
    } else if ua_lower.contains("tablet") || ua_lower.contains("ipad") {
        Some("tablet".to_string())
    } else {
        Some("desktop".to_string())
    }
}

fn parse_browser(ua_lower: &str) -> Option<String> {
    if ua_lower.contains("edg/") || ua_lower.contains("edge") {
        Some("Edge".to_string())
    } else if ua_lower.contains("samsungbrowser") {
        Some("Samsung Internet".to_string())
    } else if ua_lower.contains("ucbrowser") || ua_lower.contains("ucweb") {
        Some("UC Browser".to_string())
    } else if ua_lower.contains("yabrowser") {
        Some("Yandex".to_string())
    } else if ua_lower.contains("qqbrowser") {
        Some("QQ Browser".to_string())
    } else if ua_lower.contains("micromessenger") {
        Some("WeChat".to_string())
    } else if ua_lower.contains("silk/") {
        Some("Silk".to_string())
    } else if ua_lower.contains("electron") {
        Some("Electron".to_string())
    } else if ua_lower.contains("cordova") || ua_lower.contains("wv)") {
        Some("WebView".to_string())
    } else if ua_lower.contains("chrome") && !ua_lower.contains("edg") {
        Some("Chrome".to_string())
    } else if ua_lower.contains("firefox") {
        Some("Firefox".to_string())
    } else if ua_lower.contains("safari") && !ua_lower.contains("chrome") {
        Some("Safari".to_string())
    } else if ua_lower.contains("opera") || ua_lower.contains("opr/") {
        Some("Opera".to_string())
    } else {
        None
    }
}

fn parse_os(ua_lower: &str) -> Option<String> {
    if ua_lower.contains("windows") {
        Some("Windows".to_string())
    } else if ua_lower.contains("mac os x") || ua_lower.contains("macos") {
        Some("macOS".to_string())
    } else if ua_lower.contains("linux") {
        Some("Linux".to_string())
    } else if ua_lower.contains("android") {
        Some("Android".to_string())
    } else if ua_lower.contains("iphone") || ua_lower.contains("ipad") || ua_lower.contains("ios") {
        Some("iOS".to_string())
    } else {
        None
    }
}
