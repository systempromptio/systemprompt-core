use systemprompt_models::wire::sse::frame_end;

#[test]
fn lf_terminator() {
    assert_eq!(frame_end(b"data: x\n\n"), Some(9));
}

#[test]
fn crlf_terminator() {
    let buf = b"data: x\r\n\r\n";
    assert_eq!(frame_end(buf), Some(buf.len()));
}

#[test]
fn lone_cr_terminator() {
    assert_eq!(frame_end(b"data: x\r\r"), Some(9));
}

#[test]
fn mixed_lf_then_crlf() {
    let buf = b"data: x\n\r\n";
    assert_eq!(frame_end(buf), Some(buf.len()));
}

#[test]
fn mixed_crlf_then_lf() {
    let buf = b"data: x\r\n\n";
    assert_eq!(frame_end(buf), Some(buf.len()));
}

#[test]
fn reports_only_first_frame_lf() {
    let buf = b"data: a\n\ndata: b\n\n";
    let end = frame_end(buf).expect("frame");
    assert_eq!(&buf[..end], b"data: a\n\n");
}

#[test]
fn reports_only_first_frame_crlf() {
    let buf = b"data: a\r\n\r\ndata: b\r\n\r\n";
    let end = frame_end(buf).expect("frame");
    assert_eq!(&buf[..end], b"data: a\r\n\r\n");
}

#[test]
fn multiline_event_not_split_by_single_lf() {
    let buf = b"event: ping\ndata: x\n\n";
    assert_eq!(frame_end(buf), Some(buf.len()));
}

#[test]
fn multiline_event_not_split_by_single_crlf() {
    let buf = b"event: ping\r\ndata: x\r\n\r\n";
    assert_eq!(frame_end(buf), Some(buf.len()));
}

#[test]
fn incomplete_single_lf_returns_none() {
    assert_eq!(frame_end(b"data: x\n"), None);
}

#[test]
fn incomplete_single_crlf_returns_none() {
    assert_eq!(frame_end(b"data: x\r\n"), None);
}

#[test]
fn no_terminator_returns_none() {
    assert_eq!(frame_end(b"data: x"), None);
}

#[test]
fn empty_buffer_returns_none() {
    assert_eq!(frame_end(b""), None);
}

#[test]
fn leading_blank_line_is_a_frame() {
    let buf = b"\r\n\r\nrest";
    assert_eq!(frame_end(buf), Some(4));
}
