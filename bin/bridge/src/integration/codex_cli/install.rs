use std::io::Write;
use std::path::Path;

use super::config::{
    self, ANALYTICS_ENABLED, OTEL_ENDPOINT, OTEL_EXPORTER, OTEL_LOG_USER_PROMPT,
    PROVIDER_AUTH_COMMAND, PROVIDER_AUTH_REFRESH, PROVIDER_BASE_URL, PROVIDER_HEADER_TENANT,
    PROVIDER_WIRE_API, TOP_MODEL_PROVIDER,
};
use super::probe::write_dotted;
use crate::integration::host_app::{GeneratedProfile, ProfileGenInputs};

const PROVIDER_ID: &str = "systemprompt";
const MOBILECONFIG_TMPL: &str = include_str!("templates/codex_managed.mobileconfig.tmpl");

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let dir = std::env::temp_dir().join("systemprompt-bridge");
    std::fs::create_dir_all(&dir)?;
    let (payload_uuid, profile_uuid) = config::make_uuids();

    let toml_text = render_managed_toml(inputs)?;

    if cfg!(target_os = "macos") {
        let path = dir.join(format!("codex-bridge-{}.mobileconfig", config::now_unix()));
        let xml = render_mobileconfig(&toml_text, &payload_uuid, &profile_uuid);
        std::fs::File::create(&path)?.write_all(xml.as_bytes())?;
        Ok(GeneratedProfile {
            path: path.display().to_string(),
            bytes: xml.len(),
            payload_uuid,
            profile_uuid,
        })
    } else {
        let path = dir.join(format!("codex-bridge-{}-managed_config.toml", config::now_unix()));
        std::fs::File::create(&path)?.write_all(toml_text.as_bytes())?;
        Ok(GeneratedProfile {
            path: path.display().to_string(),
            bytes: toml_text.len(),
            payload_uuid,
            profile_uuid,
        })
    }
}

pub(super) fn install_profile(generated_path: &str) -> std::io::Result<()> {
    if cfg!(target_os = "macos") {
        std::process::Command::new("/usr/bin/open")
            .arg(generated_path)
            .status()?;
        Ok(())
    } else if cfg!(target_os = "windows") {
        let target = config::managed_config_path();
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        atomic_copy(generated_path.as_ref(), &target)
    } else {
        let target = config::managed_config_path();
        if let Some(parent) = target.parent() {
            if std::fs::create_dir_all(parent).is_ok() && writable(parent) {
                atomic_copy(generated_path.as_ref(), &target)?;
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!(
                        "/etc/codex is admin-owned. Copy as root: sudo install -m 0644 {generated_path} {}",
                        target.display()
                    ),
                ))
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "cannot create /etc/codex. Copy as root: sudo install -D -m 0644 {generated_path} {}",
                    target.display()
                ),
            ))
        }
    }
}

fn render_managed_toml(inputs: &ProfileGenInputs) -> std::io::Result<String> {
    let helper_bin = std::env::current_exe()?
        .canonicalize()
        .unwrap_or_else(|_| std::env::current_exe().unwrap_or_default())
        .display()
        .to_string();
    let tenant = inputs.organization_uuid.clone().unwrap_or_default();
    let gateway = inputs.gateway_base_url.trim_end_matches('/');
    let otel_endpoint = derive_otel_endpoint(gateway);

    let mut value = toml::Value::Table(toml::map::Map::new());
    write_dotted(
        &mut value,
        TOP_MODEL_PROVIDER,
        toml::Value::String(PROVIDER_ID.to_string()),
    );
    write_dotted(
        &mut value,
        &format!("model_providers.{PROVIDER_ID}.name"),
        toml::Value::String("systemprompt".to_string()),
    );
    write_dotted(
        &mut value,
        PROVIDER_BASE_URL,
        toml::Value::String(format!("{gateway}/v1")),
    );
    write_dotted(
        &mut value,
        PROVIDER_WIRE_API,
        toml::Value::String("responses".to_string()),
    );
    write_dotted(
        &mut value,
        PROVIDER_AUTH_COMMAND,
        toml::Value::String(helper_bin),
    );
    write_dotted(
        &mut value,
        "model_providers.systemprompt.auth.args",
        toml::Value::Array(vec![
            toml::Value::String("credential-helper".to_string()),
            toml::Value::String("--host".to_string()),
            toml::Value::String("codex-cli".to_string()),
        ]),
    );
    write_dotted(
        &mut value,
        "model_providers.systemprompt.auth.timeout_ms",
        toml::Value::Integer(5000),
    );
    write_dotted(
        &mut value,
        PROVIDER_AUTH_REFRESH,
        toml::Value::Integer(300_000),
    );
    if !tenant.is_empty() {
        write_dotted(
            &mut value,
            PROVIDER_HEADER_TENANT,
            toml::Value::String(tenant),
        );
    }
    write_dotted(
        &mut value,
        OTEL_EXPORTER,
        toml::Value::String("otlp-http".to_string()),
    );
    write_dotted(&mut value, OTEL_LOG_USER_PROMPT, toml::Value::Boolean(false));
    write_dotted(
        &mut value,
        OTEL_ENDPOINT,
        toml::Value::String(otel_endpoint),
    );
    write_dotted(
        &mut value,
        "otel.exporter.systemprompt.protocol",
        toml::Value::String("binary".to_string()),
    );
    write_dotted(&mut value, ANALYTICS_ENABLED, toml::Value::Boolean(false));

    if !inputs.models.is_empty() {
        let arr: Vec<toml::Value> = inputs
            .models
            .iter()
            .map(|m| toml::Value::String(m.clone()))
            .collect();
        write_dotted(
            &mut value,
            "model_providers.systemprompt.models",
            toml::Value::Array(arr),
        );
    }

    toml::to_string(&value).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn render_mobileconfig(toml_text: &str, payload_uuid: &str, profile_uuid: &str) -> String {
    let encoded = base64_encode(toml_text.as_bytes());
    MOBILECONFIG_TMPL
        .replace("{profile_uuid}", profile_uuid)
        .replace("{payload_uuid}", payload_uuid)
        .replace("{config_toml_base64}", &encoded)
}

fn derive_otel_endpoint(gateway: &str) -> String {
    if let Some(host_part) = gateway.strip_prefix("https://") {
        return format!("https://{}/otel", host_part.trim_end_matches('/'));
    }
    if let Some(host_part) = gateway.strip_prefix("http://") {
        return format!("http://{}/otel", host_part.trim_end_matches('/'));
    }
    format!("{gateway}/otel")
}

fn atomic_copy(source: &Path, target: &Path) -> std::io::Result<()> {
    let pid = std::process::id();
    let tmp = target.with_extension(format!(
        "{}.tmp.{pid}",
        target
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("toml")
    ));
    std::fs::copy(source, &tmp)?;
    std::fs::rename(&tmp, target)?;
    Ok(())
}

fn writable(path: &Path) -> bool {
    let probe = path.join(format!(".systemprompt-bridge-write-test-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        },
        Err(_) => false,
    }
}

fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    let mut chunks = input.chunks_exact(3);
    for chunk in &mut chunks {
        let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
        out.push(CHARS[((n >> 18) & 0x3f) as usize] as char);
        out.push(CHARS[((n >> 12) & 0x3f) as usize] as char);
        out.push(CHARS[((n >> 6) & 0x3f) as usize] as char);
        out.push(CHARS[(n & 0x3f) as usize] as char);
    }
    let rem = chunks.remainder();
    match rem.len() {
        1 => {
            let n = (rem[0] as u32) << 16;
            out.push(CHARS[((n >> 18) & 0x3f) as usize] as char);
            out.push(CHARS[((n >> 12) & 0x3f) as usize] as char);
            out.push('=');
            out.push('=');
        },
        2 => {
            let n = ((rem[0] as u32) << 16) | ((rem[1] as u32) << 8);
            out.push(CHARS[((n >> 18) & 0x3f) as usize] as char);
            out.push(CHARS[((n >> 12) & 0x3f) as usize] as char);
            out.push(CHARS[((n >> 6) & 0x3f) as usize] as char);
            out.push('=');
        },
        _ => {},
    }
    out
}
