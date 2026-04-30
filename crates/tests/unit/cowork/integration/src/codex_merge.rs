use std::fs;
use std::sync::{Mutex, MutexGuard};

use systemprompt_bridge::integration::host_app::ProfileGenInputs;
use systemprompt_bridge::integration::{HostApp, find_host_by_id};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn fresh_codex_home() -> (std::path::PathBuf, MutexGuard<'static, ()>) {
    let guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let dir = tempfile::tempdir().expect("tempdir").keep();
    unsafe {
        std::env::set_var("CODEX_HOME", &dir);
    }
    (dir, guard)
}

fn codex_inputs() -> ProfileGenInputs {
    ProfileGenInputs {
        gateway_base_url: "https://gateway.systemprompt.io".to_string(),
        api_key: "sp-test-key".to_string(),
        models: vec!["claude-opus-4-7".to_string()],
        organization_uuid: Some("org-abc".to_string()),
    }
}

#[test]
fn generated_managed_toml_contains_required_keys() {
    let (_home, _lock) = fresh_codex_home();
    let host = find_host_by_id("codex-cli").expect("codex host registered");
    let profile = host.generate_profile(&codex_inputs()).expect("generate");
    assert!(!profile.path.is_empty());
    assert!(profile.bytes > 0);

    let raw = fs::read_to_string(&profile.path).expect("read generated profile");
    let toml_text = if profile.path.ends_with(".mobileconfig") {
        extract_base64_from_mobileconfig(&raw)
    } else {
        raw
    };

    assert!(toml_text.contains("model_provider = \"systemprompt\""));
    assert!(toml_text.contains("base_url = \"https://gateway.systemprompt.io/v1\""));
    assert!(toml_text.contains("wire_api = \"responses\""));
    assert!(toml_text.contains("[model_providers.systemprompt.auth]"));
    assert!(toml_text.contains("credential-helper"));
    assert!(toml_text.contains("[otel"));
    assert!(toml_text.contains("enabled = false"));
}

#[test]
fn generated_managed_toml_includes_organization_tenant_header() {
    let (_home, _lock) = fresh_codex_home();
    let host = find_host_by_id("codex-cli").expect("codex host registered");
    let profile = host.generate_profile(&codex_inputs()).expect("generate");
    let raw = fs::read_to_string(&profile.path).expect("read");
    let toml_text = if profile.path.ends_with(".mobileconfig") {
        extract_base64_from_mobileconfig(&raw)
    } else {
        raw
    };
    assert!(
        toml_text.contains("x-tenant = \"org-abc\""),
        "expected x-tenant header in managed TOML, got: {toml_text}"
    );
}

#[test]
fn install_on_writable_target_writes_managed_config() {
    if cfg!(target_os = "macos") {
        return;
    }
    if !cfg!(target_os = "windows") {
        if !std::path::Path::new("/etc/codex").exists() && std::env::var("USER").as_deref() != Ok("root") {
            return;
        }
    }
    let (home, _lock) = fresh_codex_home();
    let host = find_host_by_id("codex-cli").expect("codex host registered");
    let profile = host.generate_profile(&codex_inputs()).expect("generate");
    if let Err(e) = host.install_profile(&profile.path) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("install: {e}");
    }
    if cfg!(target_os = "windows") {
        let written =
            fs::read_to_string(home.join("managed_config.toml")).expect("managed_config exists");
        assert!(written.contains("model_provider = \"systemprompt\""));
    }
}

fn extract_base64_from_mobileconfig(xml: &str) -> String {
    let needle = "<key>config_toml_base64</key>";
    let after = xml.split(needle).nth(1).expect("mobileconfig has config_toml_base64");
    let start = after.find("<string>").expect("string open") + "<string>".len();
    let end = after[start..].find("</string>").expect("string close");
    let b64 = &after[start..start + end];
    let bytes = base64_decode(b64.trim());
    String::from_utf8(bytes).expect("base64 decoded utf-8")
}

fn base64_decode(input: &str) -> Vec<u8> {
    fn val(c: u8) -> i16 {
        match c {
            b'A'..=b'Z' => (c - b'A') as i16,
            b'a'..=b'z' => (c - b'a' + 26) as i16,
            b'0'..=b'9' => (c - b'0' + 52) as i16,
            b'+' => 62,
            b'/' => 63,
            b'=' => -1,
            _ => -2,
        }
    }
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let bytes: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let v: [i16; 4] = [val(chunk[0]), val(chunk[1]), val(chunk[2]), val(chunk[3])];
        let n = ((v[0].max(0) as u32) << 18)
            | ((v[1].max(0) as u32) << 12)
            | ((v[2].max(0) as u32) << 6)
            | (v[3].max(0) as u32);
        out.push(((n >> 16) & 0xff) as u8);
        if v[2] != -1 {
            out.push(((n >> 8) & 0xff) as u8);
        }
        if v[3] != -1 {
            out.push((n & 0xff) as u8);
        }
    }
    out
}
