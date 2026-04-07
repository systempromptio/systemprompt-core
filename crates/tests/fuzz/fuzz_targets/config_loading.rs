#![no_main]
use libfuzzer_sys::fuzz_target;
use systemprompt_models::profile::Profile;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_yaml::from_str::<Profile>(s);
    }
});
