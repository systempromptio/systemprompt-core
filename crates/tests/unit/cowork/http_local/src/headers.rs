use systemprompt_bridge::http_local::parse_from_read;

#[test]
fn header_lookup_is_case_insensitive() {
    let raw = b"GET / HTTP/1.1\r\nX-FoO: yes\r\nHost: x\r\n\r\n";
    let req = parse_from_read(&raw[..]).expect("valid");
    assert_eq!(req.header("x-foo"), Some("yes"));
    assert_eq!(req.header("X-FOO"), Some("yes"));
}

#[test]
fn header_without_colon_is_skipped_not_rejected() {
    let raw = b"GET / HTTP/1.1\r\nNoColonHere\r\nHost: x\r\n\r\n";
    let req = parse_from_read(&raw[..]).expect("malformed line skipped, request still valid");
    assert_eq!(req.header("host"), Some("x"));
}

#[test]
fn whitespace_around_value_is_trimmed() {
    let raw = b"GET / HTTP/1.1\r\nX-Pad:    value-here   \r\n\r\n";
    let req = parse_from_read(&raw[..]).expect("valid");
    assert_eq!(req.header("x-pad"), Some("value-here"));
}

#[test]
fn duplicate_headers_preserved_in_order() {
    let raw = b"GET / HTTP/1.1\r\nX-A: 1\r\nX-A: 2\r\n\r\n";
    let req = parse_from_read(&raw[..]).expect("valid");
    let xa: Vec<&str> = req
        .headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("x-a"))
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(xa, vec!["1", "2"]);
}
