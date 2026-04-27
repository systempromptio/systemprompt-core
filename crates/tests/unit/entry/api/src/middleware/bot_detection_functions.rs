//! Unit tests for bot detection pure functions
//!
//! Tests cover:
//! - is_datacenter_ip with known datacenter prefixes
//! - is_datacenter_ip with regular IPs
//! - is_known_bot with various user agent strings
//! - is_outdated_browser with Chrome version detection
//! - is_scanner_request with scanner paths and agents

use systemprompt_api::services::middleware::bot_detector::{
    is_datacenter_ip, is_known_bot, is_outdated_browser, is_scanner_request,
};

#[test]
fn datacenter_ip_alibaba_prefix() {
    assert!(is_datacenter_ip(Some("47.88.1.1")));
}

#[test]
fn datacenter_ip_tencent_prefix() {
    assert!(is_datacenter_ip(Some("119.29.1.1")));
}

#[test]
fn datacenter_ip_another_alibaba() {
    assert!(is_datacenter_ip(Some("47.104.200.1")));
}

#[test]
fn datacenter_ip_regular_ip_not_detected() {
    assert!(!is_datacenter_ip(Some("192.168.1.1")));
}

#[test]
fn datacenter_ip_google_dns_not_detected() {
    assert!(!is_datacenter_ip(Some("8.8.8.8")));
}

#[test]
fn datacenter_ip_none_not_detected() {
    assert!(!is_datacenter_ip(None));
}

#[test]
fn datacenter_ip_localhost_not_detected() {
    assert!(!is_datacenter_ip(Some("127.0.0.1")));
}

#[test]
fn outdated_browser_chrome_90() {
    assert!(is_outdated_browser(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/90.0.4430.85 Safari/537.36"
    ));
}

#[test]
fn outdated_browser_chrome_119() {
    assert!(is_outdated_browser(
        "Mozilla/5.0 Chrome/119.0.0.0 Safari/537.36"
    ));
}

#[test]
fn current_browser_chrome_120() {
    assert!(!is_outdated_browser(
        "Mozilla/5.0 Chrome/120.0.0.0 Safari/537.36"
    ));
}

#[test]
fn current_browser_chrome_130() {
    assert!(!is_outdated_browser(
        "Mozilla/5.0 Chrome/130.0.0.0 Safari/537.36"
    ));
}

#[test]
fn outdated_browser_firefox_not_affected() {
    assert!(!is_outdated_browser(
        "Mozilla/5.0 (X11; Linux x86_64; rv:90.0) Gecko/20100101 Firefox/90.0"
    ));
}

#[test]
fn outdated_browser_empty_string() {
    assert!(!is_outdated_browser(""));
}

#[test]
fn outdated_browser_no_chrome() {
    assert!(!is_outdated_browser("Safari/605.1.15"));
}

#[test]
fn scanner_request_env_path() {
    assert!(is_scanner_request("/.env", "Mozilla/5.0"));
}

#[test]
fn scanner_request_git_path() {
    assert!(is_scanner_request("/.git/config", "Mozilla/5.0"));
}

#[test]
fn scanner_request_php_path() {
    assert!(is_scanner_request("/index.php", "Mozilla/5.0"));
}

#[test]
fn scanner_request_wp_admin_path() {
    assert!(is_scanner_request("/wp-admin/login", "Mozilla/5.0"));
}

#[test]
fn scanner_request_wp_login_path() {
    assert!(is_scanner_request("/wp-login.php", "Mozilla/5.0"));
}

#[test]
fn scanner_request_sql_path() {
    assert!(is_scanner_request("/db.sql", "Mozilla/5.0"));
}

#[test]
fn scanner_request_nmap_agent() {
    assert!(is_scanner_request("/", "nmap scripting engine"));
}

#[test]
fn scanner_request_sqlmap_agent() {
    assert!(is_scanner_request("/api/users", "sqlmap/1.5"));
}

#[test]
fn scanner_request_nikto_agent() {
    assert!(is_scanner_request("/", "Nikto/2.1.6"));
}

#[test]
fn scanner_request_burp_agent() {
    assert!(is_scanner_request("/", "BurpSuite/2023.1"));
}

#[test]
fn scanner_request_normal_path_and_agent() {
    assert!(!is_scanner_request(
        "/api/v1/agents",
        "Mozilla/5.0 Chrome/130.0.0.0"
    ));
}

#[test]
fn scanner_request_case_insensitive_path() {
    assert!(is_scanner_request("/WP-ADMIN", "Mozilla/5.0"));
}

#[test]
fn scanner_request_case_insensitive_agent() {
    assert!(is_scanner_request("/", "NMAP scripting"));
}

#[test]
fn known_bot_googlebot() {
    assert!(is_known_bot(
        "Googlebot/2.1 (+http://www.google.com/bot.html)"
    ));
}

#[test]
fn known_bot_bingbot() {
    assert!(is_known_bot("bingbot/2.0"));
}

#[test]
fn known_bot_regular_browser() {
    assert!(!is_known_bot(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130.0.0.0 \
         Safari/537.36"
    ));
}
