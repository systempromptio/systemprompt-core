#![no_main]
use libfuzzer_sys::fuzz_target;
use systemprompt_wire::anthropic::{buffered_defect, parse_response};

fuzz_target!(|data: &[u8]| {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };
    let _ = parse_response(&value, "fallback-model");
    let _ = buffered_defect(&value);
});
