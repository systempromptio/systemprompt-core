use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const DESKTOP_DOMAIN: &str = "com.anthropic.claudefordesktop";
const CODE_DOMAIN: &str = "com.anthropic.claudecode";

const MANAGED_PREFS_ROOT: &str = "/Library/Managed Preferences";

fn managed_prefs_candidates(domain: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(user) = std::env::var("USER") {
        if !user.is_empty() {
            out.push(
                PathBuf::from(MANAGED_PREFS_ROOT)
                    .join(&user)
                    .join(format!("{domain}.plist")),
            );
        }
    }
    out.push(PathBuf::from(MANAGED_PREFS_ROOT).join(format!("{domain}.plist")));
    out
}

const KEYS_OF_INTEREST: &[&str] = &[
    "inferenceProvider",
    "inferenceGatewayBaseUrl",
    "inferenceGatewayApiKey",
    "inferenceGatewayAuthScheme",
    "inferenceGatewayHeaders",
    "inferenceModels",
    "deploymentOrganizationUuid",
];

const DEFAULT_MODELS: &[&str] = &["claude-opus-4-7", "claude-sonnet-4-6", "claude-haiku-4-5"];

#[derive(Debug, Clone, Serialize, Default)]
pub struct ClaudeIntegrationSnapshot {
    pub managed_prefs: ManagedPrefsState,
    pub gateway_health: GatewayHealth,
    pub claude_running: bool,
    pub claude_processes: Vec<String>,
    pub probed_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ManagedPrefsState {
    pub desktop: ManagedDomain,
    pub code: ManagedDomain,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ManagedDomain {
    pub domain: String,
    pub plist_path: Option<String>,
    pub installed: bool,
    pub keys: BTreeMap<String, String>,
    pub missing_required: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct GatewayHealth {
    pub url: Option<String>,
    pub state: GatewayProbeState,
    pub http_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub enum GatewayProbeState {
    #[default]
    Unknown,
    Unconfigured,
    Listening,
    Refused,
    Timeout,
    HttpError,
}

pub fn probe() -> ClaudeIntegrationSnapshot {
    let managed_prefs = ManagedPrefsState {
        desktop: read_domain(
            DESKTOP_DOMAIN,
            &[
                "inferenceProvider",
                "inferenceGatewayBaseUrl",
                "inferenceGatewayApiKey",
                "inferenceModels",
            ],
        ),
        code: read_domain(CODE_DOMAIN, &[]),
    };

    let gateway_url = managed_prefs
        .desktop
        .keys
        .get("inferenceGatewayBaseUrl")
        .cloned();

    let gateway_health = match gateway_url.as_deref() {
        Some(url) if !url.is_empty() => probe_gateway(url),
        _ => GatewayHealth {
            url: None,
            state: GatewayProbeState::Unconfigured,
            ..Default::default()
        },
    };

    let claude_processes = list_claude_processes();
    ClaudeIntegrationSnapshot {
        managed_prefs,
        gateway_health,
        claude_running: !claude_processes.is_empty(),
        claude_processes,
        probed_at_unix: now_unix(),
    }
}

fn read_domain(domain: &str, required: &[&str]) -> ManagedDomain {
    let mut out = ManagedDomain {
        domain: domain.to_string(),
        ..Default::default()
    };

    let plist_path = managed_prefs_candidates(domain)
        .into_iter()
        .find(|p| p.exists());

    if let Some(path) = plist_path.as_ref() {
        out.plist_path = Some(path.display().to_string());
        out.installed = true;
    }

    let plist_json = plist_path
        .as_deref()
        .and_then(read_plist_as_json)
        .unwrap_or(serde_json::Value::Null);

    for key in KEYS_OF_INTEREST {
        if let Some(val) = read_key_value(&plist_json, domain, key) {
            out.keys.insert(key.to_string(), val);
        }
    }

    out.missing_required = required
        .iter()
        .filter(|k| !out.keys.contains_key(**k))
        .map(|k| (*k).to_string())
        .collect();

    out
}

fn read_plist_as_json(path: &std::path::Path) -> Option<serde_json::Value> {
    let output = Command::new("/usr/bin/plutil")
        .arg("-convert")
        .arg("json")
        .arg("-o")
        .arg("-")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

fn read_key_value(plist_json: &serde_json::Value, domain: &str, key: &str) -> Option<String> {
    if let Some(val) = plist_json.get(key) {
        return Some(format_plist_value(key, val));
    }

    let output = Command::new("/usr/bin/defaults")
        .arg("read")
        .arg(domain)
        .arg(key)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    Some(redact_if_sensitive(key, raw))
}

fn format_plist_value(key: &str, value: &serde_json::Value) -> String {
    let rendered = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    };
    redact_if_sensitive(key, rendered)
}

fn redact_if_sensitive(key: &str, raw: String) -> String {
    if key == "inferenceGatewayApiKey" {
        return format!(
            "<present, {} chars>",
            raw.chars().filter(|c| !c.is_whitespace()).count()
        );
    }
    raw
}

fn probe_gateway(url: &str) -> GatewayHealth {
    let started = Instant::now();

    let (host, port) = match parse_host_port(url) {
        Ok(v) => v,
        Err(e) => {
            return GatewayHealth {
                url: Some(url.to_string()),
                state: GatewayProbeState::HttpError,
                error: Some(e),
                ..Default::default()
            };
        },
    };

    let addr = format!("{host}:{port}");
    let resolved = match resolve_first(&addr) {
        Some(a) => a,
        None => {
            return GatewayHealth {
                url: Some(url.to_string()),
                state: GatewayProbeState::HttpError,
                error: Some(format!("cannot resolve {addr}")),
                ..Default::default()
            };
        },
    };

    let stream = match std::net::TcpStream::connect_timeout(
        &resolved,
        std::time::Duration::from_millis(1500),
    ) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            return GatewayHealth {
                url: Some(url.to_string()),
                state: GatewayProbeState::Refused,
                error: Some(e.to_string()),
                latency_ms: Some(started.elapsed().as_millis() as u64),
                ..Default::default()
            };
        },
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
            return GatewayHealth {
                url: Some(url.to_string()),
                state: GatewayProbeState::Timeout,
                error: Some(e.to_string()),
                latency_ms: Some(started.elapsed().as_millis() as u64),
                ..Default::default()
            };
        },
        Err(e) => {
            return GatewayHealth {
                url: Some(url.to_string()),
                state: GatewayProbeState::HttpError,
                error: Some(e.to_string()),
                latency_ms: Some(started.elapsed().as_millis() as u64),
                ..Default::default()
            };
        },
    };

    let latency_ms = started.elapsed().as_millis() as u64;
    let _ = stream.shutdown(std::net::Shutdown::Both);

    GatewayHealth {
        url: Some(url.to_string()),
        state: GatewayProbeState::Listening,
        http_status: None,
        latency_ms: Some(latency_ms),
        error: None,
    }
}

fn resolve_first(addr: &str) -> Option<std::net::SocketAddr> {
    use std::net::ToSocketAddrs;
    addr.to_socket_addrs().ok()?.next()
}

fn parse_host_port(raw: &str) -> Result<(String, u16), String> {
    let (scheme, rest) = match raw.split_once("://") {
        Some(v) => v,
        None => return Err(format!("missing scheme in {raw}")),
    };
    let default_port: u16 = match scheme.to_ascii_lowercase().as_str() {
        "http" => 80,
        "https" => 443,
        other => return Err(format!("unsupported scheme: {other}")),
    };
    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() {
        return Err("missing host".into());
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().unwrap_or(default_port)),
        None => (authority.to_string(), default_port),
    };
    Ok((host, port))
}

fn list_claude_processes() -> Vec<String> {
    let output = match Command::new("/bin/ps").args(["-Ao", "comm"]).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut hits: Vec<String> = text
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            (lower.contains("/claude.app/")
                || lower.ends_with("/claude")
                || lower.contains("claude helper"))
                && !lower.contains("claude code")
        })
        .map(|s| s.trim().to_string())
        .collect();
    hits.sort();
    hits.dedup();
    hits
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone)]
pub struct ProfileGenInputs {
    pub gateway_base_url: String,
    pub api_key: String,
    pub models: Vec<String>,
    pub organization_uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedProfile {
    pub path: String,
    pub bytes: usize,
    pub payload_uuid: String,
    pub profile_uuid: String,
}

pub fn default_models() -> Vec<String> {
    DEFAULT_MODELS.iter().map(|s| (*s).to_string()).collect()
}

pub fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let dir = std::env::temp_dir().join("systemprompt-cowork");
    std::fs::create_dir_all(&dir)?;
    let payload_uuid = format!(
        "ce0a{}-cwk0-4cwk-cwk0-{}",
        format!("{:08x}", now_unix() & 0xFFFF_FFFF),
        format!("{:012x}", now_unix() ^ 0xDEADBEEF_CAFEBABEu64)
    );
    let profile_uuid = format!(
        "ce0b{}-cwk0-4cwk-cwk0-{}",
        format!("{:08x}", (now_unix() ^ 0x1234_5678) & 0xFFFF_FFFF),
        format!("{:012x}", now_unix() ^ 0xFEEDFACE_DEADC0DEu64)
    );
    let path = dir.join(format!("claude-cowork-{}.mobileconfig", now_unix()));

    let xml = render_profile(inputs, &payload_uuid, &profile_uuid);
    {
        let mut f = std::fs::File::create(&path)?;
        f.write_all(xml.as_bytes())?;
    }

    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: xml.len(),
        payload_uuid,
        profile_uuid,
    })
}

fn render_profile(inputs: &ProfileGenInputs, payload_uuid: &str, profile_uuid: &str) -> String {
    let models_xml: String = inputs
        .models
        .iter()
        .map(|m| format!("            <string>{}</string>", xml_escape(m)))
        .collect::<Vec<_>>()
        .join("\n");

    let org_xml = match inputs.organization_uuid.as_deref() {
        Some(uuid) if !uuid.is_empty() => format!(
            "        <key>deploymentOrganizationUuid</key>\n        <string>{}</string>\n",
            xml_escape(uuid)
        ),
        _ => String::new(),
    };

    PROFILE_TMPL
        .replace("{profile_uuid}", &xml_escape(profile_uuid))
        .replace("{payload_uuid}", &xml_escape(payload_uuid))
        .replace("{base_url}", &xml_escape(&inputs.gateway_base_url))
        .replace("{api_key}", &xml_escape(&inputs.api_key))
        .replace("{models_xml}", &models_xml)
        .replace("{org_xml}", &org_xml)
}

const PROFILE_TMPL: &str =
    include_str!("templates/claude_desktop_profile.mobileconfig.tmpl");

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn install_profile(path: &str) -> std::io::Result<()> {
    Command::new("/usr/bin/open").arg(path).status()?;
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateProfileBody {
    #[serde(default)]
    pub gateway_base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub models: Option<Vec<String>>,
    #[serde(default)]
    pub organization_uuid: Option<String>,
}
