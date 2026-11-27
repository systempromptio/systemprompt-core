use systemprompt_engine::lsp::{FrameDecoder, FrameError, encode_frame};

const MAX_HEADER_BYTES: usize = 8 * 1024;

fn drain(decoder: &mut FrameDecoder) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    while let Some(frame) = decoder.next_frame().unwrap() {
        frames.push(frame);
    }
    frames
}

#[test]
fn round_trips_one_frame() {
    let mut decoder = FrameDecoder::default();
    decoder.push(&encode_frame(br#"{"jsonrpc":"2.0"}"#));
    assert_eq!(drain(&mut decoder), vec![br#"{"jsonrpc":"2.0"}"#.to_vec()]);
}

#[test]
fn reassembles_frames_split_at_every_byte() {
    let first = r#"{"id":1,"result":"héllo"}"#.as_bytes();
    let second = br#"{"method":"x"}"#;
    let mut stream = encode_frame(first);
    stream.extend_from_slice(&encode_frame(second));
    stream.extend_from_slice(&encode_frame(b""));

    for split in 0..=stream.len() {
        let mut decoder = FrameDecoder::default();
        let mut frames = Vec::new();
        decoder.push(&stream[..split]);
        frames.extend(drain(&mut decoder));
        decoder.push(&stream[split..]);
        frames.extend(drain(&mut decoder));
        assert_eq!(
            frames,
            vec![first.to_vec(), second.to_vec(), Vec::new()],
            "split at {split}"
        );
    }
}

#[test]
fn reassembles_byte_by_byte() {
    let stream = [encode_frame(b"[1]"), encode_frame(b"[2,3]")].concat();
    let mut decoder = FrameDecoder::default();
    let mut frames = Vec::new();
    for byte in &stream {
        decoder.push(std::slice::from_ref(byte));
        frames.extend(drain(&mut decoder));
    }
    assert_eq!(frames, vec![b"[1]".to_vec(), b"[2,3]".to_vec()]);
}

#[test]
fn accepts_extra_headers_in_any_case() {
    let mut decoder = FrameDecoder::default();
    decoder.push(
        b"content-type: application/vscode-jsonrpc; charset=utf-8\r\nCONTENT-LENGTH:  2 \r\n\r\n{}",
    );
    assert_eq!(drain(&mut decoder), vec![b"{}".to_vec()]);
}

#[test]
fn rejects_bad_headers() {
    let mut decoder = FrameDecoder::default();
    decoder.push(b"Content-Type: x\r\n\r\n{}");
    assert_eq!(decoder.next_frame(), Err(FrameError::MissingLength));

    let mut decoder = FrameDecoder::default();
    decoder.push(b"Content-Length: ten\r\n\r\n");
    assert_eq!(
        decoder.next_frame(),
        Err(FrameError::InvalidLength {
            value: "ten".into(),
        })
    );

    let mut decoder = FrameDecoder::default();
    decoder.push(b"Content-Length: 999999999999\r\n\r\n");
    assert_eq!(
        decoder.next_frame(),
        Err(FrameError::TooLarge(999_999_999_999))
    );

    let mut decoder = FrameDecoder::default();
    decoder.push(&vec![b'x'; MAX_HEADER_BYTES + 1]);
    assert_eq!(decoder.next_frame(), Err(FrameError::HeaderTooLong));

    let mut decoder = FrameDecoder::default();
    decoder.push("Content-Length: 2\r\nX-Note: é\r\n\r\n{}".as_bytes());
    assert_eq!(decoder.next_frame(), Err(FrameError::NonAsciiHeader));
}
