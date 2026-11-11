use std::time::{Duration, Instant};

use systemprompt_wire::sse::{SseFrame, SseFrameTooLarge, SseFramer};

fn frames_whole(bytes: &[u8]) -> Vec<String> {
    let mut framer = SseFramer::default();
    framer
        .push(bytes)
        .into_iter()
        .map(|f| f.expect("frame").data)
        .collect()
}

fn frames_split_at(bytes: &[u8], cut: usize) -> Vec<String> {
    let mut framer = SseFramer::default();
    let mut out: Vec<SseFrame> = Vec::new();
    for part in [&bytes[..cut], &bytes[cut..]] {
        out.extend(framer.push(part).into_iter().map(|f| f.expect("frame")));
    }
    out.into_iter().map(|f| f.data).collect()
}

fn frames_byte_by_byte(bytes: &[u8]) -> Vec<String> {
    let mut framer = SseFramer::default();
    bytes
        .iter()
        .flat_map(|b| framer.push(std::slice::from_ref(b)))
        .map(|f| f.expect("frame").data)
        .collect()
}

#[test]
fn data_field_accepts_the_value_with_or_without_the_space() {
    assert_eq!(
        frames_whole(b"data:{\"a\":1}\n\ndata: {\"b\":2}\n\n"),
        vec!["{\"a\":1}", "{\"b\":2}"]
    );
}

#[test]
fn only_one_leading_space_is_stripped() {
    assert_eq!(frames_whole(b"data:  x\n\n"), vec![" x"]);
}

#[test]
fn crlf_lf_and_lone_cr_all_end_lines() {
    assert_eq!(
        frames_whole(b"data: a\r\n\r\ndata: b\n\ndata: c\r\rdata: d\r\n\n"),
        vec!["a", "b", "c", "d"]
    );
}

#[test]
fn multi_line_data_is_joined_with_newlines() {
    assert_eq!(
        frames_whole(b"data: {\ndata: \"k\": 1\ndata: }\n\n"),
        vec!["{\n\"k\": 1\n}"]
    );
}

#[test]
fn comments_and_other_fields_are_ignored() {
    assert_eq!(
        frames_whole(b": keepalive\n\nevent: ping\nid: 7\nretry: 10\ndata: x\n\n"),
        vec!["x"]
    );
}

#[test]
fn a_frame_without_data_dispatches_nothing() {
    assert!(frames_whole(b"event: ping\n\n: c\n\n").is_empty());
}

#[test]
fn an_unterminated_frame_stays_buffered_as_the_remainder() {
    let mut framer = SseFramer::default();
    assert_eq!(framer.push(b"data: a\n\n{\"error\":").len(), 1);
    assert_eq!(framer.remainder(), b"{\"error\":");
}

#[test]
fn a_cr_at_a_chunk_end_followed_by_lf_is_one_line_end() {
    let mut framer = SseFramer::default();
    assert!(framer.push(b"data: a\r").is_empty());
    assert!(framer.push(b"\n").is_empty());
    let frames = framer.push(b"\r\n");
    assert_eq!(
        frames,
        vec![Ok(SseFrame {
            data: "a".to_owned()
        })]
    );
}

#[test]
fn every_single_cut_yields_the_whole_stream_frames() {
    let stream = "data: {\"t\":\"héllo — 世界\"}\r\n\r\n: c\ndata:{\"n\":2}\n\ndata: x\r\rdata: a\ndata: b\r\n\n"
        .as_bytes();
    let whole = frames_whole(stream);
    assert_eq!(whole.len(), 4);
    assert_eq!(whole[0], "{\"t\":\"héllo — 世界\"}");
    for cut in 0..=stream.len() {
        assert_eq!(frames_split_at(stream, cut), whole, "cut at {cut}");
    }
    assert_eq!(frames_byte_by_byte(stream), whole);
}

#[test]
fn a_frame_past_the_cap_is_rejected_and_the_framer_stops() {
    let mut framer = SseFramer::with_max_frame_bytes(16);
    assert!(framer.push(b"data: 0123456789").is_empty());
    assert_eq!(
        framer.push(b"abcdef"),
        vec![Err(SseFrameTooLarge { limit: 16 })]
    );
    assert!(framer.push(b"\n\ndata: x\n\n").is_empty());
}

#[test]
fn frames_before_an_oversized_one_in_the_same_chunk_are_kept() {
    let mut framer = SseFramer::with_max_frame_bytes(16);
    let out = framer.push(b"data: ok\n\ndata: 0123456789abcdef\n");
    assert_eq!(
        out,
        vec![
            Ok(SseFrame {
                data: "ok".to_owned()
            }),
            Err(SseFrameTooLarge { limit: 16 })
        ]
    );
}

#[test]
fn many_small_frames_in_one_chunk_do_not_count_against_the_cap() {
    let mut framer = SseFramer::with_max_frame_bytes(16);
    let stream = "data: x\n\n".repeat(100);
    let out = framer.push(stream.as_bytes());
    assert_eq!(out.len(), 100);
    assert!(out.iter().all(Result::is_ok));
}

#[test]
fn a_pathological_unterminated_line_is_rejected_at_the_cap_in_linear_time() {
    const CAP: usize = 4 * 1024 * 1024;
    let mut framer = SseFramer::with_max_frame_bytes(CAP);
    let chunk = vec![b'a'; 256];
    assert!(framer.push(b"data: ").is_empty());
    let started = Instant::now();
    let mut pushed = 6;
    let rejected = loop {
        let out = framer.push(&chunk);
        pushed += chunk.len();
        if let Some(result) = out.into_iter().next() {
            break result;
        }
        assert!(pushed <= CAP, "no rejection past the cap");
    };
    assert_eq!(rejected, Err(SseFrameTooLarge { limit: CAP }));
    assert!(pushed > CAP && pushed <= CAP + chunk.len());
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "scan took {:?}",
        started.elapsed()
    );
}

fn raw_split_at(bytes: &[u8], cut: usize) -> (Vec<Vec<u8>>, Vec<u8>) {
    let mut framer = SseFramer::default();
    let mut frames = Vec::new();
    for part in [&bytes[..cut], &bytes[cut..]] {
        frames.extend(
            framer
                .push_raw(part)
                .into_iter()
                .map(|f| f.expect("frame").raw),
        );
    }
    (frames, framer.remainder().to_vec())
}

#[test]
fn raw_frames_are_the_exact_input_bytes_at_every_cut() {
    let stream =
        ": c\r\n\r\nevent: ping\n\ndata: {\"t\":\"世界\"}\r\rdata: a\r\n\r\ndata: tail".as_bytes();
    let (whole, rest) = raw_split_at(stream, 0);
    assert_eq!(whole.len(), 4);
    assert_eq!(whole[1], b"event: ping\n\n");
    assert_eq!(rest, b"data: tail");
    for cut in 0..=stream.len() {
        let (frames, rest) = raw_split_at(stream, cut);
        let mut joined: Vec<u8> = frames.concat();
        joined.extend_from_slice(&rest);
        assert_eq!(joined, stream, "cut at {cut}");
        assert_eq!(frames.len(), 4, "cut at {cut}");
    }
}

#[test]
fn raw_frames_are_capped_like_decoded_frames() {
    let mut framer = SseFramer::with_max_frame_bytes(16);
    let out: Vec<Result<Vec<u8>, SseFrameTooLarge>> = framer
        .push_raw(b": ok\n\n: 0123456789abcdefgh")
        .into_iter()
        .map(|f| f.map(|f| f.raw))
        .collect();
    assert_eq!(
        out,
        vec![
            Ok(b": ok\n\n".to_vec()),
            Err(SseFrameTooLarge { limit: 16 })
        ]
    );
    assert!(framer.push_raw(b"\n\n").is_empty());
}

#[test]
fn raw_frames_carry_their_decoded_data() {
    let mut framer = SseFramer::default();
    let frames: Vec<_> = framer
        .push_raw(b": c\n\ndata:a\r\ndata: b\n\n")
        .into_iter()
        .map(|f| f.expect("frame").data)
        .collect();
    assert_eq!(frames, vec![None, Some("a\nb".to_owned())]);
}
