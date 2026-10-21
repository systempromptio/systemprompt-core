#![no_main]
use libfuzzer_sys::fuzz_target;
use systemprompt_wire::openai_responses::{buffered_defect, parse_response_object};

fuzz_target!(|data: &[u8]| {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };
    let _ = parse_response_object(&value, "fallback-model");
    let _ = buffered_defect(&value);
});
