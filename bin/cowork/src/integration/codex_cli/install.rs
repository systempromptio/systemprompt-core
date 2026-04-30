use std::io::Write;
use std::path::{Path, PathBuf};

use super::config::{
    self, ANALYTICS_ENABLED, OTEL_ENDPOINT, OTEL_EXPORTER, OTEL_LOG_USER_PROMPT,
    PROVIDER_AUTH_COMMAND, PROVIDER_AUTH_REFRESH, PROVIDER_BASE_URL, PROVIDER_HEADER_TENANT,
    PROVIDER_WIRE_API, TOP_MODEL_PROVIDER,
};
use super::probe::{read_dotted, write_dotted};
use crate::integration::host_app::{GeneratedProfile, ProfileGenInputs};

const BACKUP_RETENTION: usize = 3;
const PROVIDER_ID: &str = "systemprompt";

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let dir = std::env::temp_dir().join("systemprompt-bridge");
    std::fs::create_dir_all(&dir)?;
    let (payload_uuid, profile_uuid) = config::make_uuids();
    let path = dir.join(format!("codex-bridge-{}.toml", config::now_unix()));

    let toml_text = render_fragment(inputs)?;
    std::fs::File::create(&path)?.write_all(toml_text.as_bytes())?;

    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: toml_text.len(),
        payload_uuid,
        profile_uuid,
    })
}

pub(super) fn install_profile(fragment_path: &str) -> std::io::Result<()> {
    let fragment_text = std::fs::read_to_string(fragment_path)?;
    let fragment: toml::Value = toml::from_str(&fragment_text)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    let target_path = config::config_path();
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut existing: toml::Value = if target_path.exists() {
        let text = std::fs::read_to_string(&target_path)?;
        backup_existing(&target_path, &text)?;
        prune_backups(&target_path)?;
        toml::from_str(&text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    if let Some(existing_provider) = read_dotted(&existing, TOP_MODEL_PROVIDER) {
        if !matches!(&existing_provider, toml::Value::String(s) if s == PROVIDER_ID) {
            write_dotted(
                &mut existing,
                "model_provider_cowork_previous",
                existing_provider,
            );
        }
    }

    for dotted in config::KEYS_OF_INTEREST {
        if let Some(value) = read_dotted(&fragment, dotted) {
            write_dotted(&mut existing, dotted, value);
        }
    }

    if let Some(provider_table) = read_dotted(
        &fragment,
        &format!("model_providers.{PROVIDER_ID}.name"),
    ) {
        write_dotted(
            &mut existing,
            &format!("model_providers.{PROVIDER_ID}.name"),
            provider_table,
        );
    }

    let serialized = toml::to_string(&existing)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    atomic_write(&target_path, &serialized)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&target_path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&target_path, perms)?;
    }

    Ok(())
}

fn render_fragment(inputs: &ProfileGenInputs) -> std::io::Result<String> {
    let helper_bin = std::env::current_exe()?
        .canonicalize()?
        .display()
        .to_string();
    let tenant = inputs.organization_uuid.clone().unwrap_or_default();
    let gateway = inputs.gateway_base_url.trim_end_matches('/');
    let otel_endpoint = derive_otel_endpoint(gateway);

    let mut root = toml::map::Map::new();
    root.insert(
        TOP_MODEL_PROVIDER.to_string(),
        toml::Value::String(PROVIDER_ID.to_string()),
    );

    let mut value = toml::Value::Table(root);
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

fn derive_otel_endpoint(gateway: &str) -> String {
    if let Some(host_part) = gateway.strip_prefix("https://") {
        return format!("https://{}/otel", host_part.trim_end_matches('/'));
    }
    if let Some(host_part) = gateway.strip_prefix("http://") {
        return format!("http://{}/otel", host_part.trim_end_matches('/'));
    }
    format!("{gateway}/otel")
}

fn backup_existing(target: &Path, text: &str) -> std::io::Result<()> {
    let stamp = config::now_unix();
    let backup_name = format!(
        "{}.bridge-backup-{stamp}",
        target.file_name().and_then(|s| s.to_str()).unwrap_or("config.toml"),
    );
    let backup_path = target.with_file_name(backup_name);
    std::fs::write(backup_path, text)
}

fn prune_backups(target: &Path) -> std::io::Result<()> {
    let dir = match target.parent() {
        Some(d) => d,
        None => return Ok(()),
    };
    let stem = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("config.toml");
    let prefix = format!("{stem}.bridge-backup-");

    let mut backups: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.starts_with(&prefix))
        })
        .collect();
    backups.sort();

    while backups.len() > BACKUP_RETENTION {
        let oldest = backups.remove(0);
        let _ = std::fs::remove_file(oldest);
    }
    Ok(())
}

fn atomic_write(target: &Path, contents: &str) -> std::io::Result<()> {
    let pid = std::process::id();
    let tmp = target.with_extension(format!("toml.tmp.{pid}"));
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, target)
}
