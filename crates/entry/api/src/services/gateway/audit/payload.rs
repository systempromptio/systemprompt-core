use bytes::Bytes;
use serde_json::Value;

const PAYLOAD_CAP_BYTES: usize = 256 * 1024;
const EXCERPT_BYTES: usize = 8 * 1024;

pub fn slice_payload(bytes: &Bytes) -> (Option<Value>, Option<String>, bool, i32) {
    let len = bytes.len();
    let len_i32 = len.min(i32::MAX as usize) as i32;
    if len <= PAYLOAD_CAP_BYTES {
        serde_json::from_slice::<Value>(bytes).map_or_else(
            |_| {
                let excerpt = String::from_utf8_lossy(bytes).to_string();
                (None, Some(excerpt), false, len_i32)
            },
            |v| (Some(v), None, false, len_i32),
        )
    } else {
        let head_len = EXCERPT_BYTES.min(len);
        let head = String::from_utf8_lossy(&bytes[..head_len]).to_string();
        let tail_start = len.saturating_sub(EXCERPT_BYTES);
        let tail = String::from_utf8_lossy(&bytes[tail_start..]).to_string();
        let excerpt = format!("{head}\n...<truncated {} bytes>...\n{tail}", len - head_len);
        (None, Some(excerpt), true, len_i32)
    }
}

pub fn truncate_for_tool_input(input: &str) -> String {
    const TOOL_INPUT_CAP: usize = 64 * 1024;
    if input.len() <= TOOL_INPUT_CAP {
        input.to_owned()
    } else {
        // Why: `&input[..TOOL_INPUT_CAP]` panics when the cap lands inside a
        // multi-byte UTF-8 codepoint. Walk back to the nearest char boundary
        // before slicing so non-ASCII tool inputs cannot crash audit logging.
        let mut cut = TOOL_INPUT_CAP;
        while cut > 0 && !input.is_char_boundary(cut) {
            cut -= 1;
        }
        let head = &input[..cut];
        format!("{head}...<truncated {} bytes>", input.len() - cut)
    }
}
