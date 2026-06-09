use std::path::Path;

const SCANNER_EXTENSIONS: &[&str] = &[
    "php", "env", "git", "sql", "bak", "old", "zip", "gz", "db", "config", "cgi", "htm",
];

const SCANNER_PATHS: &[&str] = &[
    "/admin",
    "/wp-admin",
    "/wp-content",
    "/uploads",
    "/cgi-bin",
    "/phpmyadmin",
    "/xmlrpc",
    "/luci",
    "/ssi.cgi",
    "internal_forms_authentication",
    "/identity",
    "/login.htm",
    "/manager/html",
    "/config/",
    "/setup.cgi",
    "/eval-stdin.php",
    "/shell.php",
    "/c99.php",
];

const MIN_USER_AGENT_LENGTH: usize = 10;
const MIN_CHROME_VERSION: i32 = 120;
const MIN_FIREFOX_VERSION: i32 = 120;
const MAX_REQUESTS_PER_MINUTE: f64 = 30.0;
const MAX_CURL_UA_LENGTH: usize = 20;
const MAX_WGET_UA_LENGTH: usize = 20;
const MAX_PYTHON_REQUESTS_UA_LENGTH: usize = 30;
const MAX_GO_HTTP_CLIENT_UA_LENGTH: usize = 30;
const MAX_RUBY_UA_LENGTH: usize = 25;

const SCANNER_NEEDLES: &[&str] = &[
    "masscan",
    "nmap",
    "nikto",
    "sqlmap",
    "havij",
    "acunetix",
    "nessus",
    "openvas",
    "w3af",
    "metasploit",
    "burpsuite",
    "zap",
    "zgrab",
    "censys",
    "shodan",
    "palo alto",
    "cortex",
    "xpanse",
    "probe-image-size",
    "libredtail",
    "httpclient",
    "httpunit",
    "java/",
    "wp-http",
    "wp-cron",
];

const SHORT_UA_NEEDLES: &[(&str, usize)] = &[
    ("curl", MAX_CURL_UA_LENGTH),
    ("wget", MAX_WGET_UA_LENGTH),
    ("python-requests", MAX_PYTHON_REQUESTS_UA_LENGTH),
    ("go-http-client", MAX_GO_HTTP_CLIENT_UA_LENGTH),
    ("ruby", MAX_RUBY_UA_LENGTH),
];

#[derive(Debug, Clone, Copy)]
pub struct ScannerDetector;

impl ScannerDetector {
    #[must_use]
    pub fn is_scanner_path(path: &str) -> bool {
        Self::has_scanner_extension(path) || Self::has_scanner_directory(path)
    }

    fn has_scanner_extension(path: &str) -> bool {
        Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                SCANNER_EXTENSIONS
                    .iter()
                    .any(|scanner_ext| ext.eq_ignore_ascii_case(scanner_ext))
            })
    }

    fn has_scanner_directory(path: &str) -> bool {
        let path_lower = path.to_lowercase();
        SCANNER_PATHS.iter().any(|p| path_lower.contains(p))
    }

    #[must_use]
    pub fn is_scanner_agent(user_agent: &str) -> bool {
        let ua_lower = user_agent.to_lowercase();

        if user_agent.is_empty() || user_agent.len() < MIN_USER_AGENT_LENGTH {
            return true;
        }

        if user_agent == "Mozilla/5.0" || user_agent.trim() == "Mozilla/5.0" {
            return true;
        }

        SCANNER_NEEDLES.iter().any(|n| ua_lower.contains(n))
            || ua_lower.starts_with("wordpress/")
            || SHORT_UA_NEEDLES
                .iter()
                .any(|(needle, max_len)| ua_lower.contains(needle) && ua_lower.len() < *max_len)
            || Self::is_outdated_browser(&ua_lower)
    }

    fn is_outdated_browser(ua_lower: &str) -> bool {
        if let Some(pos) = ua_lower.find("chrome/")
            && let Some(dot_pos) = ua_lower[pos + 7..].find('.')
            && let Ok(major) = ua_lower[pos + 7..][..dot_pos].parse::<i32>()
            && major < MIN_CHROME_VERSION
        {
            return true;
        }

        if let Some(pos) = ua_lower.find("firefox/")
            && let Some(space_pos) = ua_lower[pos + 8..].find(|c: char| !c.is_numeric() && c != '.')
            && let Ok(major) = ua_lower[pos + 8..][..space_pos].parse::<i32>()
            && major < MIN_FIREFOX_VERSION
        {
            return true;
        }

        false
    }

    #[must_use]
    pub fn is_high_velocity(request_count: i64, duration_seconds: i64) -> bool {
        if duration_seconds < 1 {
            return false;
        }

        let requests_per_minute = (request_count as f64 / duration_seconds as f64) * 60.0;
        requests_per_minute > MAX_REQUESTS_PER_MINUTE
    }

    #[must_use]
    pub fn is_scanner(
        path: Option<&str>,
        user_agent: Option<&str>,
        request_count: Option<i64>,
        duration_seconds: Option<i64>,
    ) -> bool {
        if let Some(p) = path
            && Self::is_scanner_path(p)
        {
            return true;
        }

        match user_agent {
            Some(ua) => {
                if Self::is_scanner_agent(ua) {
                    return true;
                }
            },
            None => {
                return true;
            },
        }

        if let (Some(count), Some(duration)) = (request_count, duration_seconds)
            && Self::is_high_velocity(count, duration)
        {
            return true;
        }

        false
    }
}
