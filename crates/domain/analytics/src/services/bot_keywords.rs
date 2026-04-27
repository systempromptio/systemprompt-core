pub const BOT_KEYWORDS: &[&str] = &[
    "bot",
    "crawler",
    "spider",
    "scraper",
    "crawling",
    "googlebot",
    "google-inspectiontool",
    "adsbot-google",
    "googleother",
    "bingbot",
    "bingpreview",
    "msnbot",
    "baiduspider",
    "yandexbot",
    "yandex.com/bots",
    "duckduckbot",
    "slurp",
    "yahoo",
    "facebookexternalhit",
    "facebookcatalog",
    "facebot",
    "meta-externalagent",
    "twitterbot",
    "linkedinbot",
    "slackbot",
    "discordbot",
    "whatsapp",
    "telegrambot",
    "pinterestbot",
    "petalbot",
    "sogou",
    "applebot",
    "dotbot",
    "semrushbot",
    "ahrefsbot",
    "majesticbot",
    "mj12bot",
    "rogerbot",
    "exabot",
    "sistrix",
    "seolyt",
    "barkrowler",
    "blexbot",
    "bubing",
    "cliqzbot",
    "uptimerobot",
    "pingdom",
    "statuscake",
    "site24x7",
    "lighthouse",
    "pagespeed",
    "speedcurve",
    "headless",
    "phantom",
    "selenium",
    "webdriver",
    "puppeteer",
    "archive.org_bot",
    "ia_archiver",
    "embedly",
    "flipboard",
    "google-structured-data-testing-tool",
    "scrapy",
    "python-requests",
    "python-urllib",
    "python-httpx",
    "httpx/",
    "aiohttp",
    "curl",
    "wget",
    "libwww",
    "http.rb",
    "guzzlehttp",
    "okhttp",
    "apache-httpclient",
    "go-http-client",
    "node-fetch",
    "axios",
    "java/",
    "perl/",
    "ruby/",
    "playstore-google",
    "google-read-aloud",
    "prefetch proxy",
    "surf/",
    "amazon-quick",
    "gzip(gfe)",
    "rv:1.9",
];

pub const BOT_IP_PREFIXES: &[&str] = &[
    "66.249.", "40.77.", "157.55.", "207.46.", "69.171.", "173.252.", "31.13.",
];

pub fn is_malformed_user_agent(user_agent: &str) -> bool {
    if user_agent.is_empty() || user_agent.len() < 10 {
        return true;
    }

    let trimmed = user_agent.trim();
    if trimmed.starts_with('{') || trimmed.ends_with('}') {
        return true;
    }

    let lower = trimmed.to_lowercase();
    matches!(lower.as_str(), "-" | "null" | "unknown")
}

pub fn matches_bot_pattern(user_agent: &str) -> bool {
    if is_malformed_user_agent(user_agent) {
        return true;
    }

    let ua_lower = user_agent.to_lowercase();

    if BOT_KEYWORDS
        .iter()
        .any(|keyword| ua_lower.contains(keyword))
    {
        return true;
    }

    if ua_lower.contains("compatible")
        && !ua_lower.contains("chrome")
        && !ua_lower.contains("firefox")
        && !ua_lower.contains("safari")
        && !ua_lower.contains("edge")
    {
        return true;
    }

    false
}

pub fn matches_bot_ip_range(ip: &str) -> bool {
    BOT_IP_PREFIXES.iter().any(|prefix| ip.starts_with(prefix))
}
