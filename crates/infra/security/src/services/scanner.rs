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

#[derive(Debug, Clone, Copy)]
pub struct ScannerDetector;

impl ScannerDetector {
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

    pub fn is_scanner_agent(user_agent: &str) -> bool {
        let ua_lower = user_agent.to_lowercase();

        if user_agent.is_empty() || user_agent.len() < 10 {
            return true;
        }

        if user_agent == "Mozilla/5.0" || user_agent.trim() == "Mozilla/5.0" {
            return true;
        }

        ua_lower.contains("masscan")
            || ua_lower.contains("nmap")
            || ua_lower.contains("nikto")
            || ua_lower.contains("sqlmap")
            || ua_lower.contains("havij")
            || ua_lower.contains("acunetix")
            || ua_lower.contains("nessus")
            || ua_lower.contains("openvas")
            || ua_lower.contains("w3af")
            || ua_lower.contains("metasploit")
            || ua_lower.contains("burpsuite")
            || ua_lower.contains("zap")
            || ua_lower.contains("zgrab")
            || ua_lower.contains("censys")
            || ua_lower.contains("shodan")
            || ua_lower.contains("masscan")
            || ua_lower.contains("palo alto")
            || ua_lower.contains("cortex")
            || ua_lower.contains("xpanse")
            || ua_lower.contains("probe-image-size")
            || ua_lower.contains("libredtail")
            || ua_lower.contains("httpclient")
            || ua_lower.contains("httpunit")
            || ua_lower.contains("java/")
            || ua_lower.starts_with("wordpress/")
            || ua_lower.contains("wp-http")
            || ua_lower.contains("wp-cron")
            || (ua_lower.contains("curl") && ua_lower.len() < 20)
            || (ua_lower.contains("wget") && ua_lower.len() < 20)
            || (ua_lower.contains("python-requests") && ua_lower.len() < 30)
            || (ua_lower.contains("go-http-client") && ua_lower.len() < 30)
            || (ua_lower.contains("ruby") && ua_lower.len() < 25)
            || Self::is_outdated_browser(&ua_lower)
    }

    fn is_outdated_browser(ua_lower: &str) -> bool {
        if ua_lower.contains("chrome/") {
            if let Some(pos) = ua_lower.find("chrome/") {
                let version_str = &ua_lower[pos + 7..];
                if let Some(dot_pos) = version_str.find('.') {
                    if let Ok(major) = version_str[..dot_pos].parse::<i32>() {
                        if major < 90 {
                            return true;
                        }
                    }
                }
            }
        }

        if ua_lower.contains("firefox/") {
            if let Some(pos) = ua_lower.find("firefox/") {
                let version_str = &ua_lower[pos + 8..];
                if let Some(space_pos) = version_str.find(|c: char| !c.is_numeric() && c != '.') {
                    if let Ok(major) = version_str[..space_pos].parse::<i32>() {
                        if major < 88 {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    pub fn is_high_velocity(request_count: i64, duration_seconds: i64) -> bool {
        if duration_seconds < 1 {
            return false;
        }

        let requests_per_minute = (request_count as f64 / duration_seconds as f64) * 60.0;
        requests_per_minute > 30.0
    }

    pub fn is_scanner(
        path: Option<&str>,
        user_agent: Option<&str>,
        request_count: Option<i64>,
        duration_seconds: Option<i64>,
    ) -> bool {
        if let Some(p) = path {
            if Self::is_scanner_path(p) {
                return true;
            }
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

        if let (Some(count), Some(duration)) = (request_count, duration_seconds) {
            if Self::is_high_velocity(count, duration) {
                return true;
            }
        }

        false
    }
}
