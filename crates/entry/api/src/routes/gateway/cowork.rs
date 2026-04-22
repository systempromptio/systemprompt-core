use axum::{
    Extension, Json,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use base64::Engine;
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use systemprompt_identifiers::headers as sp_headers;
use systemprompt_mcp::services::RegistryManager;
use systemprompt_models::{AppPaths, SecretsBootstrap};
use systemprompt_runtime::AppContext;

#[derive(Serialize)]
struct PluginFileEntry {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Serialize)]
struct PluginEntry {
    id: String,
    version: String,
    sha256: String,
    files: Vec<PluginFileEntry>,
}

#[derive(Serialize)]
struct ManagedMcpServer {
    name: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    transport: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    oauth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_policy: Option<BTreeMap<String, String>>,
}

pub async fn pubkey() -> impl IntoResponse {
    match signing_key() {
        Ok(key) => {
            let vk: VerifyingKey = key.verifying_key();
            let b64 = base64::engine::general_purpose::STANDARD.encode(vk.to_bytes());
            (StatusCode::OK, Json(json!({ "pubkey": b64 }))).into_response()
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

pub async fn manifest(
    Extension(ctx): Extension<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let _ = ctx;
    let user_id = headers
        .get(sp_headers::USER_ID)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("anonymous")
        .to_string();
    let tenant_id = headers
        .get(sp_headers::TENANT_ID)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let plugins = match enumerate_plugins() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("plugin enumeration failed: {e}") })),
            )
                .into_response();
        },
    };

    let mcp_servers = match enumerate_mcp_servers() {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("mcp registry load failed: {e}") })),
            )
                .into_response();
        },
    };

    let manifest_version = format!(
        "{}-{}",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
        &short_hash_json(&plugins, &mcp_servers)
    );
    let issued_at = chrono::Utc::now().to_rfc3339();

    let payload = json!({
        "manifest_version": manifest_version,
        "issued_at": issued_at,
        "user_id": user_id,
        "tenant_id": tenant_id,
        "plugins": plugins,
        "managed_mcp_servers": mcp_servers,
        "revocations": Vec::<String>::new(),
    });

    let canonical = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("serialize payload: {e}") })),
            )
                .into_response();
        },
    };

    let signing = match signing_key() {
        Ok(k) => k,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e })),
            )
                .into_response();
        },
    };

    let signature = signing.sign(canonical.as_bytes());
    let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_bytes());

    let mut response = payload;
    if let Some(obj) = response.as_object_mut() {
        obj.insert("signature".into(), json!(signature_b64));
    }

    (StatusCode::OK, Json(response)).into_response()
}

fn signing_key() -> Result<&'static SigningKey, String> {
    static CELL: OnceLock<SigningKey> = OnceLock::new();
    if let Some(k) = CELL.get() {
        return Ok(k);
    }
    let secret =
        SecretsBootstrap::jwt_secret().map_err(|e| format!("jwt secret unavailable: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(b"systemprompt-cowork-manifest-ed25519-v1");
    hasher.update(secret.as_bytes());
    let seed: [u8; 32] = hasher.finalize().into();
    let key = SigningKey::from_bytes(&seed);
    let _ = CELL.set(key.clone());
    Ok(CELL.get().expect("just set"))
}

fn enumerate_plugins() -> Result<Vec<PluginEntry>, String> {
    let paths = AppPaths::get().map_err(|e| e.to_string())?;
    let plugins_root = paths.system().services().join("plugins");
    if !plugins_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    let entries = std::fs::read_dir(&plugins_root).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(id) = name.to_str() else { continue };
        if id.starts_with('.') {
            continue;
        }
        let plugin_dir = entry.path();
        let files = collect_plugin_files(&plugin_dir)?;
        let dir_hash = directory_hash_from_files(&files);
        let version = read_plugin_version(&plugin_dir).unwrap_or_else(|| "0.0.0".into());
        out.push(PluginEntry {
            id: id.to_string(),
            version,
            sha256: dir_hash,
            files,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

const BLOCKED: &[&str] = &["config.yaml", "config.yml"];

fn collect_plugin_files(root: &Path) -> Result<Vec<PluginFileEntry>, String> {
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn walk(base: &Path, dir: &Path, out: &mut Vec<PluginFileEntry>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        let path = entry.path();
        if ft.is_dir() {
            walk(base, &path, out)?;
        } else if ft.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if BLOCKED.contains(&name) {
                    continue;
                }
            }
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            let mut h = Sha256::new();
            h.update(&bytes);
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(PluginFileEntry {
                path: rel,
                sha256: hex_encode(&h.finalize()),
                size: bytes.len() as u64,
            });
        }
    }
    Ok(())
}

fn directory_hash_from_files(files: &[PluginFileEntry]) -> String {
    let mut hasher = Sha256::new();
    for f in files {
        hasher.update(f.path.as_bytes());
        hasher.update(b"\0");
        if let Ok(decoded) = hex_decode(&f.sha256) {
            hasher.update(&decoded);
        }
        hasher.update(b"\0");
    }
    hex_encode(&hasher.finalize())
}

fn read_plugin_version(plugin_dir: &Path) -> Option<String> {
    let candidates = [
        plugin_dir.join("claude-plugin").join("version.json"),
        plugin_dir.join("claude-plugin").join("plugin.json"),
        plugin_dir.join("plugin.json"),
    ];
    for p in &candidates {
        if let Ok(bytes) = std::fs::read(p) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some(v) = value.get("version").and_then(|x| x.as_str()) {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn enumerate_mcp_servers() -> Result<Vec<ManagedMcpServer>, String> {
    let configs = RegistryManager::get_enabled_servers()
        .map_err(|e| format!("RegistryManager error: {e}"))?;
    let mut out = Vec::with_capacity(configs.len());
    for cfg in configs {
        let endpoint = systemprompt_models::modules::ApiPaths::mcp_server_endpoint(&cfg.name);
        out.push(ManagedMcpServer {
            name: cfg.name.clone(),
            url: endpoint,
            transport: Some("http".into()),
            headers: None,
            oauth: Some(cfg.oauth.required),
            tool_policy: None,
        });
    }
    Ok(out)
}

fn short_hash_json<T: Serialize, U: Serialize>(a: &[T], b: &[U]) -> String {
    let mut h = Sha256::new();
    if let Ok(s) = serde_json::to_string(a) {
        h.update(s.as_bytes());
    }
    h.update(b"|");
    if let Ok(s) = serde_json::to_string(b) {
        h.update(s.as_bytes());
    }
    let digest = h.finalize();
    hex_encode(&digest[..4])
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd hex length".into());
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex byte: {b}")),
    }
}

#[allow(dead_code)]
fn _ensure_pathbuf_used(_: PathBuf) {}
