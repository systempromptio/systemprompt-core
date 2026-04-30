use systemprompt_bridge::http_local::parse_from_read;

#[test]
fn chunked_body_decodes() {
    let raw = b"POST /x HTTP/1.1\r\n\
                Host: x\r\n\
                Transfer-Encoding: chunked\r\n\
                \r\n\
                5\r\nhello\r\n\
                6\r\n world\r\n\
                0\r\n\r\n";
    let req = parse_from_read(&raw[..]).expect("valid chunked");
    assert_eq!(req.body, b"hello world");
}

#[test]
fn chunked_with_extension_after_size_decodes() {
    let raw = b"POST /x HTTP/1.1\r\n\
                Host: x\r\n\
                Transfer-Encoding: chunked\r\n\
                \r\n\
                5;name=val\r\nhello\r\n\
                0\r\n\r\n";
    let req = parse_from_read(&raw[..]).expect("chunk extension must be tolerated");
    assert_eq!(req.body, b"hello");
}

#[test]
fn chunked_truncated_size_line_rejected() {
    let raw = b"POST /x HTTP/1.1\r\n\
                Host: x\r\n\
                Transfer-Encoding: chunked\r\n\
                \r\n\
                5\r\nhel";
    let err = parse_from_read(&raw[..]).expect_err("truncated chunk body must reject");
    let _ = format!("{err}");
}

#[test]
fn chunked_bad_size_radix_rejected() {
    let raw = b"POST /x HTTP/1.1\r\n\
                Host: x\r\n\
                Transfer-Encoding: chunked\r\n\
                \r\n\
                ZZZ\r\nhello\r\n\
                0\r\n\r\n";
    let err = parse_from_read(&raw[..]).expect_err("non-hex chunk size must reject");
    let _ = format!("{err}");
}

#[test]
fn chunked_premature_eof_after_size_rejected() {
    let raw = b"POST /x HTTP/1.1\r\n\
                Host: x\r\n\
                Transfer-Encoding: chunked\r\n\
                \r\n";
    let err = parse_from_read(&raw[..]).expect_err("premature EOF after headers must reject");
    let _ = format!("{err}");
}
