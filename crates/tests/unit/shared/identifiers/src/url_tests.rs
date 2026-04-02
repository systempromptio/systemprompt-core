use systemprompt_identifiers::{ValidatedUrl, DbValue, ToDbValue};

#[test]
fn valid_https_url() {
    let url = ValidatedUrl::try_new("https://example.com").unwrap();
    assert_eq!(url.as_str(), "https://example.com");
}

#[test]
fn valid_http_url() {
    let url = ValidatedUrl::try_new("http://example.com/path").unwrap();
    assert_eq!(url.as_str(), "http://example.com/path");
}

#[test]
fn valid_url_with_port() {
    let url = ValidatedUrl::try_new("https://example.com:8080/api").unwrap();
    assert_eq!(url.as_str(), "https://example.com:8080/api");
}

#[test]
fn valid_url_with_query_and_fragment() {
    let url = ValidatedUrl::try_new("https://example.com/path?key=value#section").unwrap();
    assert_eq!(url.as_str(), "https://example.com/path?key=value#section");
}

#[test]
fn valid_url_with_userinfo() {
    let url = ValidatedUrl::try_new("https://user:pass@example.com/path").unwrap();
    assert_eq!(url.as_str(), "https://user:pass@example.com/path");
}

#[test]
fn valid_custom_scheme() {
    let url = ValidatedUrl::try_new("ftp://files.example.com/pub").unwrap();
    assert_eq!(url.scheme(), "ftp");
}

#[test]
fn valid_scheme_with_plus() {
    let url = ValidatedUrl::try_new("coap+tcp://example.com/resource").unwrap();
    assert_eq!(url.scheme(), "coap+tcp");
}

#[test]
fn rejects_empty_string() {
    let err = ValidatedUrl::try_new("").unwrap_err();
    assert_eq!(err.to_string(), "ValidatedUrl cannot be empty");
}

#[test]
fn rejects_no_scheme() {
    let err = ValidatedUrl::try_new("example.com").unwrap_err();
    assert!(err.to_string().contains("scheme"));
}

#[test]
fn rejects_empty_scheme() {
    let err = ValidatedUrl::try_new("://example.com").unwrap_err();
    assert!(err.to_string().contains("scheme cannot be empty"));
}

#[test]
fn rejects_scheme_starting_with_digit() {
    let err = ValidatedUrl::try_new("1http://example.com").unwrap_err();
    assert!(err.to_string().contains("must start with a letter"));
}

#[test]
fn rejects_scheme_with_invalid_chars() {
    let err = ValidatedUrl::try_new("ht tp://example.com").unwrap_err();
    assert!(err.to_string().contains("invalid characters"));
}

#[test]
fn rejects_no_host() {
    let err = ValidatedUrl::try_new("https://").unwrap_err();
    assert!(err.to_string().contains("host"));
}

#[test]
fn rejects_empty_host_for_non_file() {
    let err = ValidatedUrl::try_new("http://:8080/path").unwrap_err();
    assert!(err.to_string().contains("host cannot be empty"));
}

#[test]
fn scheme_extraction() {
    let url = ValidatedUrl::new("https://example.com");
    assert_eq!(url.scheme(), "https");
}

#[test]
fn is_https_true() {
    let url = ValidatedUrl::new("https://example.com");
    assert!(url.is_https());
}

#[test]
fn is_https_false_for_http() {
    let url = ValidatedUrl::new("http://example.com");
    assert!(!url.is_https());
}

#[test]
fn is_http_true_for_http() {
    let url = ValidatedUrl::new("http://example.com");
    assert!(url.is_http());
}

#[test]
fn is_http_true_for_https() {
    let url = ValidatedUrl::new("https://example.com");
    assert!(url.is_http());
}

#[test]
fn is_http_false_for_ftp() {
    let url = ValidatedUrl::new("ftp://example.com");
    assert!(!url.is_http());
}

#[test]
fn is_https_case_insensitive() {
    let url = ValidatedUrl::new("HTTPS://EXAMPLE.COM");
    assert!(url.is_https());
}

#[test]
fn display_shows_full_url() {
    let url = ValidatedUrl::new("https://example.com/path");
    assert_eq!(format!("{}", url), "https://example.com/path");
}

#[test]
fn serde_roundtrip_exact_json() {
    let url = ValidatedUrl::new("https://example.com");
    let json = serde_json::to_string(&url).unwrap();
    assert_eq!(json, "\"https://example.com\"");
    let deserialized: ValidatedUrl = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, url);
}

#[test]
fn serde_rejects_invalid_on_deserialize() {
    let result: Result<ValidatedUrl, _> = serde_json::from_str("\"not-a-url\"");
    assert!(result.is_err());
}

#[test]
fn try_from_str_ref() {
    let url: ValidatedUrl = "https://example.com".try_into().unwrap();
    assert_eq!(url.as_str(), "https://example.com");
}

#[test]
fn try_from_string() {
    let url: ValidatedUrl = String::from("https://example.com").try_into().unwrap();
    assert_eq!(url.as_str(), "https://example.com");
}

#[test]
fn from_str_parse() {
    let url: ValidatedUrl = "https://example.com".parse().unwrap();
    assert_eq!(url.as_str(), "https://example.com");
}

#[test]
fn to_db_value_returns_string_variant() {
    let url = ValidatedUrl::new("https://example.com");
    let db_val = url.to_db_value();
    assert!(matches!(db_val, DbValue::String(s) if s == "https://example.com"));
}

#[test]
fn equality_across_construction_paths() {
    let from_new = ValidatedUrl::new("https://example.com");
    let from_try: ValidatedUrl = "https://example.com".try_into().unwrap();
    let from_parse: ValidatedUrl = "https://example.com".parse().unwrap();
    assert_eq!(from_new, from_try);
    assert_eq!(from_try, from_parse);
}

#[test]
#[should_panic(expected = "ValidatedUrl validation failed")]
fn new_panics_on_invalid() {
    let _ = ValidatedUrl::new("not-a-url");
}

#[test]
fn valid_ipv6_url() {
    let url = ValidatedUrl::try_new("https://[::1]/path").unwrap();
    assert_eq!(url.as_str(), "https://[::1]/path");
}

#[test]
fn rejects_empty_ipv6() {
    let err = ValidatedUrl::try_new("https://[]/path").unwrap_err();
    assert!(err.to_string().contains("IPv6"));
}

#[test]
fn rejects_empty_port_after_colon() {
    let err = ValidatedUrl::try_new("https://example.com:/path").unwrap_err();
    assert!(err.to_string().contains("port"));
}
