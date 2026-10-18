#![no_main]
use libfuzzer_sys::fuzz_target;
use systemprompt_gateway::protocol::inbound::openai_chat::parse::parse;

fuzz_target!(|data: &[u8]| {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };
    let _ = parse(&value);
});
