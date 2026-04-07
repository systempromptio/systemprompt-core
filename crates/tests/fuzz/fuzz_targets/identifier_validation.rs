#![no_main]
use libfuzzer_sys::fuzz_target;
use systemprompt_identifiers::{AgentName, Email};

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = Email::try_new(s);
        let _ = AgentName::try_new(s);
    }
});
