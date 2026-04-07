#![no_main]
use libfuzzer_sys::fuzz_target;
use systemprompt_agent::models::a2a::protocol::A2aJsonRpcRequest;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<A2aJsonRpcRequest>(s);
    }
});
