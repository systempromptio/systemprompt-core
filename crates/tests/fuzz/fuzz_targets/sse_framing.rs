#![no_main]
use libfuzzer_sys::fuzz_target;
use systemprompt_wire::inspect::{SurfaceBudget, sse_string_leaves, string_leaves};
use systemprompt_wire::sse::{frame_end, upstream_error_message};

fuzz_target!(|data: &[u8]| {
    if let Some(end) = frame_end(data) {
        assert!(end <= data.len());
    }
    let _ = string_leaves(data, SurfaceBudget::default());
    let _ = sse_string_leaves(data, SurfaceBudget::default());
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) {
        let _ = upstream_error_message(&value);
    }
});
