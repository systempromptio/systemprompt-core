pub fn parse_user_agent(ua: &str) -> (Option<String>, Option<String>, Option<String>) {
    let ua_lower = ua.to_lowercase();

    let device_type = Some(parse_device_type(&ua_lower));
    let browser = Some(parse_browser(&ua_lower));
    let os = Some(parse_os(&ua_lower));

    (device_type, browser, os)
}

fn parse_device_type(ua_lower: &str) -> String {
    if ua_lower.contains("mobile") || ua_lower.contains("android") || ua_lower.contains("iphone") {
        "mobile".to_string()
    } else if ua_lower.contains("tablet") || ua_lower.contains("ipad") {
        "tablet".to_string()
    } else {
        "desktop".to_string()
    }
}

fn parse_browser(ua_lower: &str) -> String {
    if ua_lower.contains("edg/") || ua_lower.contains("edge") {
        "Edge".to_string()
    } else if ua_lower.contains("samsungbrowser") {
        "Samsung Internet".to_string()
    } else if ua_lower.contains("ucbrowser") || ua_lower.contains("ucweb") {
        "UC Browser".to_string()
    } else if ua_lower.contains("yabrowser") {
        "Yandex".to_string()
    } else if ua_lower.contains("qqbrowser") {
        "QQ Browser".to_string()
    } else if ua_lower.contains("micromessenger") {
        "WeChat".to_string()
    } else if ua_lower.contains("silk/") {
        "Silk".to_string()
    } else if ua_lower.contains("electron") {
        "Electron".to_string()
    } else if ua_lower.contains("cordova") || ua_lower.contains("wv)") {
        "WebView".to_string()
    } else if ua_lower.contains("chrome") && !ua_lower.contains("edg") {
        "Chrome".to_string()
    } else if ua_lower.contains("firefox") {
        "Firefox".to_string()
    } else if ua_lower.contains("safari") && !ua_lower.contains("chrome") {
        "Safari".to_string()
    } else if ua_lower.contains("opera") || ua_lower.contains("opr/") {
        "Opera".to_string()
    } else if ua_lower.contains("msie") || ua_lower.contains("trident") {
        "IE".to_string()
    } else if ua_lower.contains("brave") {
        "Brave".to_string()
    } else if ua_lower.contains("vivaldi") {
        "Vivaldi".to_string()
    } else if ua_lower.contains("duckduckgo") {
        "DuckDuckGo".to_string()
    } else if ua_lower.contains("arc/") {
        "Arc".to_string()
    } else {
        "Other".to_string()
    }
}

fn parse_os(ua_lower: &str) -> String {
    if ua_lower.contains("windows") {
        "Windows".to_string()
    } else if ua_lower.contains("mac os x") || ua_lower.contains("macos") {
        "macOS".to_string()
    } else if ua_lower.contains("android") {
        "Android".to_string()
    } else if ua_lower.contains("iphone") || ua_lower.contains("ipad") || ua_lower.contains("ios") {
        "iOS".to_string()
    } else if ua_lower.contains("linux") {
        "Linux".to_string()
    } else if ua_lower.contains("cros") || ua_lower.contains("chrome os") {
        "ChromeOS".to_string()
    } else if ua_lower.contains("freebsd") || ua_lower.contains("openbsd") {
        "BSD".to_string()
    } else {
        "Other".to_string()
    }
}
