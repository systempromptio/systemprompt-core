use systemprompt_agent::models::web::{extract_port_from_url, is_valid_version};

#[test]
fn test_valid_version_standard_semver() {
    assert!(is_valid_version("1.0.0"));
}

#[test]
fn test_valid_version_zero_prefix() {
    assert!(is_valid_version("0.1.0"));
}

#[test]
fn test_valid_version_all_zeros() {
    assert!(is_valid_version("0.0.0"));
}

#[test]
fn test_valid_version_large_numbers() {
    assert!(is_valid_version("100.200.300"));
}

#[test]
fn test_valid_version_single_digits() {
    assert!(is_valid_version("1.2.3"));
}

#[test]
fn test_invalid_version_two_parts() {
    assert!(!is_valid_version("1.0"));
}

#[test]
fn test_invalid_version_four_parts() {
    assert!(!is_valid_version("1.0.0.0"));
}

#[test]
fn test_invalid_version_single_number() {
    assert!(!is_valid_version("1"));
}

#[test]
fn test_invalid_version_empty_string() {
    assert!(!is_valid_version(""));
}

#[test]
fn test_invalid_version_with_prefix_v() {
    assert!(!is_valid_version("v1.0.0"));
}

#[test]
fn test_invalid_version_with_prerelease() {
    assert!(!is_valid_version("1.0.0-alpha"));
}

#[test]
fn test_invalid_version_with_build_metadata() {
    assert!(!is_valid_version("1.0.0+build"));
}

#[test]
fn test_invalid_version_letters_in_parts() {
    assert!(!is_valid_version("a.b.c"));
}

#[test]
fn test_invalid_version_negative_numbers() {
    assert!(!is_valid_version("-1.0.0"));
}

#[test]
fn test_invalid_version_spaces() {
    assert!(!is_valid_version("1. 0. 0"));
}

#[test]
fn test_invalid_version_trailing_dot() {
    assert!(!is_valid_version("1.0.0."));
}

#[test]
fn test_invalid_version_leading_dot() {
    assert!(!is_valid_version(".1.0.0"));
}

#[test]
fn test_extract_port_http_with_port() {
    assert_eq!(extract_port_from_url("http://localhost:8080"), Some(8080));
}

#[test]
fn test_extract_port_https_with_port() {
    assert_eq!(
        extract_port_from_url("https://example.com:9443"),
        Some(9443)
    );
}

#[test]
fn test_extract_port_http_without_port() {
    assert_eq!(extract_port_from_url("http://example.com"), Some(80));
}

#[test]
fn test_extract_port_https_without_port() {
    assert_eq!(extract_port_from_url("https://example.com"), Some(443));
}

#[test]
fn test_extract_port_http_with_path() {
    assert_eq!(
        extract_port_from_url("http://localhost:3000/api/v1"),
        Some(3000)
    );
}

#[test]
fn test_extract_port_https_with_path_no_port() {
    assert_eq!(
        extract_port_from_url("https://example.com/api/v1/agents"),
        Some(443)
    );
}

#[test]
fn test_extract_port_no_protocol() {
    assert_eq!(extract_port_from_url("localhost:8080"), None);
}

#[test]
fn test_extract_port_empty_string() {
    assert_eq!(extract_port_from_url(""), None);
}

#[test]
fn test_extract_port_relative_path() {
    assert_eq!(extract_port_from_url("/api/v1/agents"), None);
}

#[test]
fn test_extract_port_ftp_protocol() {
    assert_eq!(extract_port_from_url("ftp://example.com:21"), None);
}

#[test]
fn test_extract_port_http_standard_port_explicit() {
    assert_eq!(extract_port_from_url("http://example.com:80"), Some(80));
}

#[test]
fn test_extract_port_https_standard_port_explicit() {
    assert_eq!(extract_port_from_url("https://example.com:443"), Some(443));
}

#[test]
fn test_extract_port_high_port() {
    assert_eq!(extract_port_from_url("http://localhost:65535"), Some(65535));
}

#[test]
fn test_extract_port_invalid_port_number() {
    assert_eq!(extract_port_from_url("http://localhost:notaport"), None);
}

#[test]
fn test_extract_port_ip_address_with_port() {
    assert_eq!(extract_port_from_url("http://192.168.1.1:8080"), Some(8080));
}

#[test]
fn test_extract_port_ip_address_without_port() {
    assert_eq!(extract_port_from_url("http://192.168.1.1"), Some(80));
}

#[test]
fn test_extract_port_port_one() {
    assert_eq!(extract_port_from_url("http://localhost:1"), Some(1));
}

#[test]
fn test_extract_port_overflow_port() {
    assert_eq!(extract_port_from_url("http://localhost:99999"), None);
}

#[test]
fn test_valid_version_max_u32_values() {
    assert!(is_valid_version("4294967295.4294967295.4294967295"));
}
