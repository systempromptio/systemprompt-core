//! Render the bridge-owned config block into Codex-compatible TOML, and on
//! macOS wrap that TOML inside a signed-style `.mobileconfig` payload so the
//! system installer can register it under managed preferences.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

use super::super::config::{
    ANALYTICS_ENABLED, OTEL_ENDPOINT, OTEL_LOG_USER_PROMPT, OTEL_PROTOCOL, PROVIDER_AUTH_COMMAND,
    PROVIDER_AUTH_REFRESH, PROVIDER_BASE_URL, PROVIDER_HEADER_TENANT, PROVIDER_WIRE_API,
    TOP_MODEL_PROVIDER,
};
use super::super::probe::write_dotted;
use crate::integration::host_app::ProfileGenInputs;

const PROVIDER_ID: &str = "systemprompt";
const MOBILECONFIG_TMPL: &str = include_str!("../templates/codex_managed.mobileconfig.tmpl");

pub(super) fn managed_toml(inputs: &ProfileGenInputs) -> std::io::Result<String> {
    let helper_bin = std::env::current_exe()?
        .canonicalize()
        .unwrap_or_else(|_| std::env::current_exe().unwrap_or_default())
        .display()
        .to_string();
    let tenant = inputs.organization_uuid.clone().unwrap_or_default();
    let gateway = inputs.gateway_base_url.trim_end_matches('/');

    let mut value = toml::Value::Table(toml::map::Map::new());
    write_provider_block(&mut value, &helper_bin, &tenant, gateway);
    write_otel_block(&mut value, gateway);
    write_models_block(&mut value, &inputs.models);

    toml::to_string(&value).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "{profile_uuid}/{payload_uuid}/{config_toml_base64} are template placeholders \
              consumed by str::replace"
)]
pub(super) fn mobileconfig(toml_text: &str, payload_uuid: &str, profile_uuid: &str) -> String {
    let encoded = BASE64.encode(toml_text.as_bytes());
    MOBILECONFIG_TMPL
        .replace("{profile_uuid}", profile_uuid)
        .replace("{payload_uuid}", payload_uuid)
        .replace("{config_toml_base64}", &encoded)
}

fn write_provider_block(value: &mut toml::Value, helper_bin: &str, tenant: &str, gateway: &str) {
    write_dotted(
        value,
        TOP_MODEL_PROVIDER,
        toml::Value::String(PROVIDER_ID.to_owned()),
    );
    write_dotted(
        value,
        &format!("model_providers.{PROVIDER_ID}.name"),
        toml::Value::String("systemprompt".to_owned()),
    );
    write_dotted(
        value,
        PROVIDER_BASE_URL,
        toml::Value::String(format!("{gateway}/v1")),
    );
    write_dotted(
        value,
        PROVIDER_WIRE_API,
        toml::Value::String("responses".to_owned()),
    );
    write_dotted(
        value,
        PROVIDER_AUTH_COMMAND,
        toml::Value::String(helper_bin.to_owned()),
    );
    write_dotted(
        value,
        "model_providers.systemprompt.auth.args",
        toml::Value::Array(vec![
            toml::Value::String("credential-helper".to_owned()),
            toml::Value::String("--host".to_owned()),
            toml::Value::String("codex-cli".to_owned()),
        ]),
    );
    write_dotted(
        value,
        "model_providers.systemprompt.auth.timeout_ms",
        toml::Value::Integer(5000),
    );
    write_dotted(value, PROVIDER_AUTH_REFRESH, toml::Value::Integer(300_000));
    if !tenant.is_empty() {
        write_dotted(
            value,
            PROVIDER_HEADER_TENANT,
            toml::Value::String(tenant.to_owned()),
        );
    }
}

fn write_otel_block(value: &mut toml::Value, gateway: &str) {
    write_dotted(value, OTEL_LOG_USER_PROMPT, toml::Value::Boolean(false));
    write_dotted(
        value,
        OTEL_ENDPOINT,
        toml::Value::String(derive_otel_endpoint(gateway)),
    );
    write_dotted(
        value,
        OTEL_PROTOCOL,
        toml::Value::String("binary".to_owned()),
    );
    write_dotted(value, ANALYTICS_ENABLED, toml::Value::Boolean(false));
}

fn write_models_block(value: &mut toml::Value, models: &[String]) {
    if models.is_empty() {
        return;
    }
    let arr: Vec<toml::Value> = models
        .iter()
        .map(|m| toml::Value::String(m.clone()))
        .collect();
    write_dotted(
        value,
        "model_providers.systemprompt.models",
        toml::Value::Array(arr),
    );
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
