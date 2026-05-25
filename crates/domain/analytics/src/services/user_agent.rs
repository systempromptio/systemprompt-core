pub(crate) fn parse_user_agent(ua: &str) -> (Option<String>, Option<String>, Option<String>) {
    let ua_lower = ua.to_lowercase();

    let device_type = Some(parse_device_type(&ua_lower));
    let browser = Some(parse_browser(&ua_lower));
    let os = Some(parse_os(&ua_lower));

    (device_type, browser, os)
}

fn parse_device_type(ua_lower: &str) -> String {
    const MOBILE: &[&str] = &["mobile", "android", "iphone"];
    const TABLET: &[&str] = &["tablet", "ipad"];
    if MOBILE.iter().any(|s| ua_lower.contains(s)) {
        "mobile".to_owned()
    } else if TABLET.iter().any(|s| ua_lower.contains(s)) {
        "tablet".to_owned()
    } else {
        "desktop".to_owned()
    }
}

struct BrowserRule {
    name: &'static str,
    needles: &'static [&'static str],
    negative: &'static [&'static str],
}

const BROWSER_RULES: &[BrowserRule] = &[
    BrowserRule {
        name: "Edge",
        needles: &["edg/", "edge"],
        negative: &[],
    },
    BrowserRule {
        name: "Samsung Internet",
        needles: &["samsungbrowser"],
        negative: &[],
    },
    BrowserRule {
        name: "UC Browser",
        needles: &["ucbrowser", "ucweb"],
        negative: &[],
    },
    BrowserRule {
        name: "Yandex",
        needles: &["yabrowser"],
        negative: &[],
    },
    BrowserRule {
        name: "QQ Browser",
        needles: &["qqbrowser"],
        negative: &[],
    },
    BrowserRule {
        name: "WeChat",
        needles: &["micromessenger"],
        negative: &[],
    },
    BrowserRule {
        name: "Silk",
        needles: &["silk/"],
        negative: &[],
    },
    BrowserRule {
        name: "Electron",
        needles: &["electron"],
        negative: &[],
    },
    BrowserRule {
        name: "WebView",
        needles: &["cordova", "wv)"],
        negative: &[],
    },
    BrowserRule {
        name: "Chrome",
        needles: &["chrome"],
        negative: &["edg"],
    },
    BrowserRule {
        name: "Firefox",
        needles: &["firefox"],
        negative: &[],
    },
    BrowserRule {
        name: "Safari",
        needles: &["safari"],
        negative: &["chrome"],
    },
    BrowserRule {
        name: "Opera",
        needles: &["opera", "opr/"],
        negative: &[],
    },
    BrowserRule {
        name: "IE",
        needles: &["msie", "trident"],
        negative: &[],
    },
    BrowserRule {
        name: "Brave",
        needles: &["brave"],
        negative: &[],
    },
    BrowserRule {
        name: "Vivaldi",
        needles: &["vivaldi"],
        negative: &[],
    },
    BrowserRule {
        name: "DuckDuckGo",
        needles: &["duckduckgo"],
        negative: &[],
    },
    BrowserRule {
        name: "Arc",
        needles: &["arc/"],
        negative: &[],
    },
];

fn parse_browser(ua_lower: &str) -> String {
    BROWSER_RULES
        .iter()
        .find(|r| {
            r.needles.iter().any(|n| ua_lower.contains(n))
                && !r.negative.iter().any(|n| ua_lower.contains(n))
        })
        .map_or_else(|| "Other".to_owned(), |r| r.name.to_owned())
}

const OS_RULES: &[(&str, &[&str])] = &[
    ("Windows", &["windows"]),
    ("macOS", &["mac os x", "macos"]),
    ("Android", &["android"]),
    ("iOS", &["iphone", "ipad", "ios"]),
    ("Linux", &["linux"]),
    ("ChromeOS", &["cros", "chrome os"]),
    ("BSD", &["freebsd", "openbsd"]),
];

fn parse_os(ua_lower: &str) -> String {
    OS_RULES
        .iter()
        .find(|(_, needles)| needles.iter().any(|n| ua_lower.contains(n)))
        .map_or_else(|| "Other".to_owned(), |(name, _)| (*name).to_owned())
}
