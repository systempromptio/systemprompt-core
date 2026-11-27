use systemprompt_engine::llm::{SseFrame, SseParser};

fn frame(event: Option<&str>, data: &str) -> SseFrame {
    SseFrame {
        event: event.map(str::to_owned),
        data: data.to_owned(),
    }
}

fn parse_in_pieces(input: &[u8], size: usize) -> Vec<SseFrame> {
    let mut parser = SseParser::default();
    let mut frames = Vec::new();
    for piece in input.chunks(size) {
        frames.extend(parser.push(piece));
    }
    frames.extend(parser.finish());
    frames
}

#[test]
fn frames_survive_every_split() {
    let input = b"event: message_start\r\ndata: {\"a\":1}\r\n\r\n: keep-alive\n\ndata: line one\ndata: line two\n\ndata: [DONE]\n\n";
    let expected = vec![
        frame(Some("message_start"), "{\"a\":1}"),
        frame(None, "line one\nline two"),
        frame(None, "[DONE]"),
    ];
    for size in 1..=input.len() {
        assert_eq!(parse_in_pieces(input, size), expected, "chunk size {size}");
    }
}

#[test]
fn lone_carriage_returns_end_lines() {
    let input = b"data: a\r\rdata: b\r\r";
    for size in 1..=input.len() {
        let expected = vec![frame(None, "a"), frame(None, "b")];
        assert_eq!(parse_in_pieces(input, size), expected, "chunk size {size}");
    }
}

#[test]
fn finish_flushes_an_unterminated_frame() {
    let mut parser = SseParser::default();
    assert_eq!(parser.push(b"data: {\"x\""), Vec::<SseFrame>::new());
    assert_eq!(parser.push(b":true}"), Vec::<SseFrame>::new());
    assert_eq!(parser.finish(), vec![frame(None, "{\"x\":true}")]);
    assert_eq!(parser.finish(), Vec::<SseFrame>::new());
}

#[test]
fn comments_and_unknown_fields_are_ignored() {
    let input = b":ok\nid: 7\nretry: 100\n\nevent: ping\n\ndata\n\ndata:no-space\n\n";
    assert_eq!(
        parse_in_pieces(input, 4),
        vec![frame(None, ""), frame(None, "no-space")]
    );
}

#[test]
fn leading_bom_is_stripped_even_when_split() {
    let input = b"\xEF\xBB\xBFdata: hi\n\n";
    for size in 1..=input.len() {
        assert_eq!(
            parse_in_pieces(input, size),
            vec![frame(None, "hi")],
            "chunk size {size}"
        );
    }
}

#[test]
fn multibyte_text_split_mid_character() {
    let input = "data: héllo ✓\n\n".as_bytes();
    assert_eq!(parse_in_pieces(input, 1), vec![frame(None, "héllo ✓")]);
}
