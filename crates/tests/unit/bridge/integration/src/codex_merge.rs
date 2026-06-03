use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_bridge::integration::find_host_by_id;
use systemprompt_bridge::integration::host_app::ProfileGenInputs;

fn with_codex_home<R>(body: impl FnOnce(&Path) -> R) -> R {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir: PathBuf = temp.path().to_path_buf();
    let home_os: OsString = dir.clone().into();
    let system_cfg_os: OsString = dir.join("system_config.toml").into();
    temp_env::with_vars(
        [
            ("CODEX_HOME", Some(&home_os)),
            // Why: managed_config_path() honours CODEX_SYSTEM_CONFIG so
            // install tests target a tempfile instead of /etc/codex/config.toml.
            ("CODEX_SYSTEM_CONFIG", Some(&system_cfg_os)),
        ],
        || body(&dir),
    )
}

fn codex_inputs() -> ProfileGenInputs {
    ProfileGenInputs {
        gateway_base_url: "https://gateway.systemprompt.io".to_string(),
        api_key: "sp-test-key".to_string(),
        models: vec!["claude-opus-4-7".to_string()],
        organization_uuid: Some("org-abc".to_string()),
        headers: Default::default(),
    }
}

#[test]
fn generated_managed_toml_contains_required_keys() {
    with_codex_home(|_home| {
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
    });
}

#[test]
fn generated_managed_toml_includes_organization_tenant_header() {
    with_codex_home(|_home| {
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
    });
}

#[test]
fn install_merges_into_codex_system_config() {
    if cfg!(target_os = "macos") {
        return;
    }
    with_codex_home(|home| {
        let host = find_host_by_id("codex-cli").expect("codex host registered");
        let profile = host.generate_profile(&codex_inputs()).expect("generate");
        host.install_profile(&profile.path).expect("install");

        let target = if cfg!(target_os = "windows") {
            home.join("managed_config.toml")
        } else {
            home.join("system_config.toml")
        };
        let written = fs::read_to_string(&target)
            .unwrap_or_else(|e| panic!("read {}: {e}", target.display()));
        assert!(
            written.contains("model_provider = \"systemprompt\""),
            "missing model_provider in: {written}"
        );
        assert!(
            written.contains("base_url = \"https://gateway.systemprompt.io/v1\""),
            "missing base_url in: {written}"
        );
    });
}

#[test]
fn install_preserves_existing_unrelated_keys_in_target() {
    if cfg!(target_os = "macos") {
        return;
    }
    with_codex_home(|home| {
        let target = if cfg!(target_os = "windows") {
            home.join("managed_config.toml")
        } else {
            home.join("system_config.toml")
        };
        fs::write(
            &target,
            "preserved_top = \"keep\"\n[unrelated_section]\nkeep_me = \
             true\n[model_providers.openai]\nname = \"openai\"\n",
        )
        .unwrap();

        let host = find_host_by_id("codex-cli").expect("codex host registered");
        let profile = host.generate_profile(&codex_inputs()).expect("generate");
        host.install_profile(&profile.path).expect("install");

        let written = fs::read_to_string(&target).expect("read merged target");
        assert!(
            written.contains("preserved_top = \"keep\""),
            "user scalar wiped: {written}"
        );
        assert!(
            written.contains("[unrelated_section]"),
            "user table wiped: {written}"
        );
        assert!(
            written.contains("keep_me = true"),
            "user nested key wiped: {written}"
        );
        assert!(
            written.contains("[model_providers.openai]"),
            "sibling provider entry wiped: {written}"
        );
        assert!(written.contains("[model_providers.systemprompt]"));
        assert!(written.contains("model_provider = \"systemprompt\""));
    });
}

#[test]
fn install_overwrites_stale_systemprompt_provider_entry() {
    if cfg!(target_os = "macos") {
        return;
    }
    with_codex_home(|home| {
        let target = if cfg!(target_os = "windows") {
            home.join("managed_config.toml")
        } else {
            home.join("system_config.toml")
        };
        fs::write(
            &target,
            "[model_providers.systemprompt]\nbase_url = \"https://stale.example/v1\"\nwire_api = \
             \"chat\"\n",
        )
        .unwrap();

        let host = find_host_by_id("codex-cli").expect("codex host registered");
        let profile = host.generate_profile(&codex_inputs()).expect("generate");
        host.install_profile(&profile.path).expect("install");

        let written = fs::read_to_string(&target).expect("read merged target");
        assert!(
            !written.contains("https://stale.example/v1"),
            "stale base_url survived: {written}"
        );
        assert!(
            written.contains("base_url = \"https://gateway.systemprompt.io/v1\""),
            "fresh base_url missing: {written}"
        );
        assert!(
            written.contains("wire_api = \"responses\""),
            "wire_api not overwritten: {written}"
        );
    });
}

fn extract_base64_from_mobileconfig(xml: &str) -> String {
    let needle = "<key>config_toml_base64</key>";
    let after = xml
        .split(needle)
        .nth(1)
        .expect("mobileconfig has config_toml_base64");
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
