use systemprompt_gateway::stream_tap::retained::{MAX_RETAINED_STREAM_BYTES, RetainedBytes};

#[test]
fn retained_bytes_stop_at_the_cap_but_count_everything_streamed() {
    let mut retained = RetainedBytes::default();
    let chunk = vec![b'z'; 1024 * 1024];
    for _ in 0..12 {
        retained.push(&chunk);
    }
    assert_eq!(retained.streamed(), 12 * 1024 * 1024);
    assert_eq!(retained.retained(), MAX_RETAINED_STREAM_BYTES);
    let kept = retained.take();
    assert_eq!(kept.len(), MAX_RETAINED_STREAM_BYTES);
    assert!(kept.iter().all(|&b| b == b'z'));
}

#[test]
fn retained_bytes_under_the_cap_are_kept_whole() {
    let mut retained = RetainedBytes::default();
    retained.push(b"data: one\n\n");
    retained.push(b"data: two\n\n");
    assert_eq!(retained.streamed(), 22);
    assert_eq!(&retained.take()[..], b"data: one\n\ndata: two\n\n");
}
