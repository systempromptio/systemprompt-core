use systemprompt_cowork::http_local::parse_from_read;

#[test]
fn parses_minimal_get() {
    let raw = b"GET /v1/health HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let req = parse_from_read(&raw[..]).expect("valid request");
    assert_eq!(req.method, "GET");
    assert_eq!(req.path, "/v1/health");
    assert_eq!(req.query, "");
    assert_eq!(req.header("host"), Some("localhost"));
    assert!(req.body.is_empty());
}

#[test]
fn target_combines_path_and_query() {
    let raw = b"GET /a?b=1&c=2 HTTP/1.1\r\nHost: x\r\n\r\n";
    let req = parse_from_read(&raw[..]).expect("valid");
    assert_eq!(req.path, "/a");
    assert_eq!(req.query, "b=1&c=2");
    assert_eq!(req.target(), "/a?b=1&c=2");
}

#[test]
fn missing_method_rejected_on_blank_line() {
    let raw = b"\r\n";
    let err = parse_from_read(&raw[..]).expect_err("blank request line must reject");
    let _ = format!("{err}");
}

#[test]
fn missing_target_rejected() {
    let raw = b"GET\r\n\r\n";
    let err = parse_from_read(&raw[..]).expect_err("missing target must reject");
    let _ = format!("{err}");
}

#[test]
fn malformed_content_length_rejected() {
    let raw = b"POST /x HTTP/1.1\r\nHost: x\r\nContent-Length: not-a-number\r\n\r\n";
    let err = parse_from_read(&raw[..]).expect_err("non-numeric content-length must reject");
    let _ = format!("{err}");
}

#[test]
fn body_read_to_content_length() {
    let raw = b"POST /x HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\n\r\nhello";
    let req = parse_from_read(&raw[..]).expect("valid");
    assert_eq!(req.body, b"hello");
}

#[test]
fn premature_eof_in_body_rejected() {
    let raw = b"POST /x HTTP/1.1\r\nHost: x\r\nContent-Length: 100\r\n\r\nshort";
    let err = parse_from_read(&raw[..]).expect_err("body shorter than content-length must reject");
    let _ = format!("{err}");
}

#[test]
fn oversized_content_length_rejected() {
    let huge = format!(
        "POST /x HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n",
        usize::MAX
    );
    let err = parse_from_read(huge.as_bytes()).expect_err("oversized content-length must reject");
    let _ = format!("{err}");
}

#[test]
fn header_injection_via_crlf_in_value_does_not_smuggle() {
    // A header value cannot smuggle a second header — anything before the next \r\n
    // belongs to the same value, and split_once(':') only splits the first ':'.
    let raw = b"GET / HTTP/1.1\r\nX-A: a\r\nX-B: b: still-b-value\r\nHost: x\r\n\r\n";
    let req = parse_from_read(&raw[..]).expect("valid");
    assert_eq!(req.header("X-B"), Some("b: still-b-value"));
}

#[test]
fn oversized_headers_rejected() {
    let mut raw = String::from("GET / HTTP/1.1\r\n");
    let big_value = "a".repeat(40 * 1024);
    raw.push_str(&format!("X-Big: {big_value}\r\n"));
    raw.push_str("\r\n");
    let err = parse_from_read(raw.as_bytes()).expect_err("oversized headers must reject");
    let _ = format!("{err}");
}
