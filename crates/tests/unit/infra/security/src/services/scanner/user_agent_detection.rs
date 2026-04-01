//! Tests for ScannerDetector user agent detection

use systemprompt_security::ScannerDetector;

// ============================================================================
// Scanner User Agent Detection - Known Scanners
// ============================================================================

#[test]
fn test_is_scanner_agent_masscan() {
    assert!(ScannerDetector::is_scanner_agent("masscan/1.0"));
    assert!(ScannerDetector::is_scanner_agent("Masscan"));
}

#[test]
fn test_is_scanner_agent_nmap() {
    assert!(ScannerDetector::is_scanner_agent("Nmap Scripting Engine"));
    assert!(ScannerDetector::is_scanner_agent("nmap"));
}

#[test]
fn test_is_scanner_agent_nikto() {
    assert!(ScannerDetector::is_scanner_agent("Nikto/2.1.6"));
    assert!(ScannerDetector::is_scanner_agent("nikto"));
}

#[test]
fn test_is_scanner_agent_sqlmap() {
    assert!(ScannerDetector::is_scanner_agent("sqlmap/1.4"));
    assert!(ScannerDetector::is_scanner_agent("SQLMap"));
}

#[test]
fn test_is_scanner_agent_acunetix() {
    assert!(ScannerDetector::is_scanner_agent(
        "Acunetix Web Vulnerability Scanner"
    ));
    assert!(ScannerDetector::is_scanner_agent("acunetix"));
}

#[test]
fn test_is_scanner_agent_nessus() {
    assert!(ScannerDetector::is_scanner_agent("Nessus SOAP"));
    assert!(ScannerDetector::is_scanner_agent("nessus"));
}

#[test]
fn test_is_scanner_agent_openvas() {
    assert!(ScannerDetector::is_scanner_agent("OpenVAS"));
    assert!(ScannerDetector::is_scanner_agent("openvas"));
}

#[test]
fn test_is_scanner_agent_burpsuite() {
    assert!(ScannerDetector::is_scanner_agent("BurpSuite Scanner"));
    assert!(ScannerDetector::is_scanner_agent("burpsuite"));
}

#[test]
fn test_is_scanner_agent_zap() {
    assert!(ScannerDetector::is_scanner_agent("OWASP ZAP"));
    assert!(ScannerDetector::is_scanner_agent("zap/2.10"));
}

#[test]
fn test_is_scanner_agent_zgrab() {
    assert!(ScannerDetector::is_scanner_agent("zgrab/0.x"));
    assert!(ScannerDetector::is_scanner_agent("ZGrab"));
}

#[test]
fn test_is_scanner_agent_censys() {
    assert!(ScannerDetector::is_scanner_agent("Censys"));
    assert!(ScannerDetector::is_scanner_agent("censys"));
}

#[test]
fn test_is_scanner_agent_shodan() {
    assert!(ScannerDetector::is_scanner_agent("Shodan"));
    assert!(ScannerDetector::is_scanner_agent("shodan"));
}

#[test]
fn test_is_scanner_agent_metasploit() {
    assert!(ScannerDetector::is_scanner_agent("Metasploit"));
    assert!(ScannerDetector::is_scanner_agent("metasploit"));
}

#[test]
fn test_is_scanner_agent_palo_alto() {
    assert!(ScannerDetector::is_scanner_agent("Palo Alto Networks"));
    assert!(ScannerDetector::is_scanner_agent("palo alto"));
}

#[test]
fn test_is_scanner_agent_cortex() {
    assert!(ScannerDetector::is_scanner_agent("Cortex XSOAR"));
    assert!(ScannerDetector::is_scanner_agent("cortex"));
}

#[test]
fn test_is_scanner_agent_xpanse() {
    assert!(ScannerDetector::is_scanner_agent("Xpanse"));
    assert!(ScannerDetector::is_scanner_agent("xpanse"));
}

// ============================================================================
// Scanner User Agent Detection - Generic Bot Indicators
// ============================================================================

#[test]
fn test_is_scanner_agent_empty() {
    assert!(ScannerDetector::is_scanner_agent(""));
}

#[test]
fn test_is_scanner_agent_too_short() {
    assert!(ScannerDetector::is_scanner_agent("short"));
    assert!(ScannerDetector::is_scanner_agent("123456789"));
}

#[test]
fn test_is_scanner_agent_bare_mozilla() {
    assert!(ScannerDetector::is_scanner_agent("Mozilla/5.0"));
    assert!(ScannerDetector::is_scanner_agent("Mozilla/5.0 "));
    assert!(ScannerDetector::is_scanner_agent(" Mozilla/5.0"));
}

#[test]
fn test_is_scanner_agent_short_curl() {
    assert!(ScannerDetector::is_scanner_agent("curl/7.68.0"));
    assert!(ScannerDetector::is_scanner_agent("curl"));
}

#[test]
fn test_is_scanner_agent_short_wget() {
    assert!(ScannerDetector::is_scanner_agent("wget/1.20"));
    assert!(ScannerDetector::is_scanner_agent("Wget"));
}

#[test]
fn test_is_scanner_agent_short_python_requests() {
    assert!(ScannerDetector::is_scanner_agent("python-requests/2.25"));
}

#[test]
fn test_is_scanner_agent_short_go_http() {
    assert!(ScannerDetector::is_scanner_agent("Go-http-client/1.1"));
}

#[test]
fn test_is_scanner_agent_java() {
    assert!(ScannerDetector::is_scanner_agent("Java/11.0.10"));
    assert!(ScannerDetector::is_scanner_agent("java/1.8.0"));
}

#[test]
fn test_is_scanner_agent_wordpress() {
    assert!(ScannerDetector::is_scanner_agent("WordPress/5.7"));
    assert!(ScannerDetector::is_scanner_agent("wp-http"));
    assert!(ScannerDetector::is_scanner_agent("wp-cron"));
}

#[test]
fn test_is_scanner_agent_httpclient() {
    assert!(ScannerDetector::is_scanner_agent("Apache-HttpClient"));
    assert!(ScannerDetector::is_scanner_agent("httpclient"));
}

#[test]
fn test_is_scanner_agent_httpunit() {
    assert!(ScannerDetector::is_scanner_agent("HttpUnit"));
    assert!(ScannerDetector::is_scanner_agent("httpunit"));
}

#[test]
fn test_is_scanner_agent_probe_image_size() {
    assert!(ScannerDetector::is_scanner_agent("probe-image-size"));
}

// ============================================================================
// Scanner User Agent Detection - Outdated Browsers
// ============================================================================

#[test]
fn test_is_scanner_agent_outdated_chrome() {
    assert!(ScannerDetector::is_scanner_agent(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/89.0.4389.82 \
         Safari/537.36"
    ));
    assert!(ScannerDetector::is_scanner_agent(
        "Mozilla/5.0 (Windows NT 10.0) Chrome/50.0.2661.102"
    ));
}

#[test]
fn test_is_scanner_agent_outdated_firefox() {
    let firefox_80 =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:80.0) Gecko/20100101 Firefox/80.0 ";
    let _ = ScannerDetector::is_scanner_agent(firefox_80);

    assert!(!ScannerDetector::is_scanner_agent(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0 "
    ));
}

// ============================================================================
// Scanner User Agent Detection - Legitimate Browsers
// ============================================================================

#[test]
fn test_is_scanner_agent_modern_chrome() {
    assert!(!ScannerDetector::is_scanner_agent(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
         Chrome/120.0.0.0 Safari/537.36"
    ));
}

#[test]
fn test_is_scanner_agent_modern_firefox() {
    assert!(!ScannerDetector::is_scanner_agent(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0"
    ));
}

#[test]
fn test_is_scanner_agent_safari() {
    assert!(!ScannerDetector::is_scanner_agent(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) \
         Version/17.0 Safari/605.1.15"
    ));
}

#[test]
fn test_is_scanner_agent_edge() {
    assert!(!ScannerDetector::is_scanner_agent(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
         Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0"
    ));
}
