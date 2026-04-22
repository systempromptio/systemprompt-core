use axum::{
    Extension, Json,
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
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
use systemprompt_identifiers::UserId;
use systemprompt_mcp::services::RegistryManager;
use systemprompt_models::auth::BEARER_PREFIX;
use systemprompt_models::{AppPaths, SecretsBootstrap};
use systemprompt_runtime::AppContext;
use systemprompt_users::UserService;

use crate::services::middleware::jwt::JwtExtractor;

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
struct SkillEntry {
    id: String,
    name: String,
    description: String,
    file_path: String,
    tags: Vec<String>,
    sha256: String,
    instructions: String,
}

#[derive(Serialize)]
struct AgentEntry {
    id: String,
    name: String,
    display_name: String,
    description: String,
    version: String,
    endpoint: String,
    enabled: bool,
    is_default: bool,
    is_primary: bool,
    provider: Option<String>,
    model: Option<String>,
    mcp_servers: Vec<String>,
    skills: Vec<String>,
    tags: Vec<String>,
    card: serde_json::Value,
}

#[derive(Serialize)]
struct UserSection {
    id: String,
    name: String,
    email: String,
    display_name: Option<String>,
    roles: Vec<String>,
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

pub async fn whoami(
    Extension(ctx): Extension<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match resolve_user_id(&headers) {
        Ok(id) => id,
        Err((status, msg)) => {
            return (status, Json(json!({ "error": msg }))).into_response();
        },
    };

    let user = match load_user_section(&ctx, &user_id).await {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e })),
            )
                .into_response();
        },
    };

    (
        StatusCode::OK,
        Json(json!({
            "user": user,
            "capabilities": ["plugins", "skills", "agents", "mcp", "user"],
        })),
    )
        .into_response()
}

pub async fn manifest(
    Extension(ctx): Extension<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match resolve_user_id(&headers) {
        Ok(id) => id,
        Err((status, msg)) => {
            return (status, Json(json!({ "error": msg }))).into_response();
        },
    };

    let user_section = match load_user_section(&ctx, &user_id).await {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e })),
            )
                .into_response();
        },
    };

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

    let skills = enumerate_skills(&ctx).await;

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

    let agents = enumerate_agents(&ctx).await;

    let manifest_version = format!(
        "{}-{}",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
        &short_hash(&plugins, &mcp_servers, &skills, &agents)
    );
    let issued_at = chrono::Utc::now().to_rfc3339();

    let payload = json!({
        "manifest_version": manifest_version,
        "issued_at": issued_at,
        "user_id": user_section.id,
        "tenant_id": serde_json::Value::Null,
        "user": user_section,
        "plugins": plugins,
        "skills": skills,
        "agents": agents,
        "managed_mcp_servers": mcp_servers,
        "revocations": revocations(),
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

fn resolve_user_id(headers: &HeaderMap) -> Result<UserId, (StatusCode, String)> {
    let auth = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "missing Authorization header".to_string(),
            )
        })?;

    let token = auth.strip_prefix(BEARER_PREFIX).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Authorization must use Bearer scheme".to_string(),
        )
    })?;

    let secret = SecretsBootstrap::jwt_secret().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("jwt secret unavailable: {e}"),
        )
    })?;
    let extractor = JwtExtractor::new(&secret);
    let ctx = extractor.extract_user_context(token.trim()).map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            format!("invalid bearer token: {e}"),
        )
    })?;
    Ok(ctx.user_id)
}

async fn load_user_section(ctx: &AppContext, user_id: &UserId) -> Result<UserSection, String> {
    let svc =
        UserService::new(ctx.db_pool()).map_err(|e| format!("user service init failed: {e}"))?;
    let user = svc
        .find_by_id(user_id)
        .await
        .map_err(|e| format!("user lookup failed: {e}"))?
        .ok_or_else(|| format!("user {} not found", user_id.as_str()))?;
    Ok(UserSection {
        id: user.id.as_str().to_string(),
        name: user.name,
        email: user.email,
        display_name: user.display_name,
        roles: user.roles,
    })
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

async fn enumerate_skills(ctx: &AppContext) -> Vec<SkillEntry> {
    let repo = match systemprompt_agent::repository::content::SkillRepository::new(ctx.db_pool()) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let skills = match repo.list_enabled().await {
        Ok(rs) => rs,
        Err(_) => return Vec::new(),
    };
    let mut out: Vec<SkillEntry> = skills
        .into_iter()
        .map(|s| {
            let mut h = Sha256::new();
            h.update(s.id.as_str().as_bytes());
            h.update(b"\0");
            h.update(s.instructions.as_bytes());
            let sha256 = hex_encode(&h.finalize());
            SkillEntry {
                id: s.id.as_str().to_string(),
                name: s.name,
                description: s.description,
                file_path: s.file_path,
                tags: s.tags,
                sha256,
                instructions: s.instructions,
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

async fn enumerate_agents(ctx: &AppContext) -> Vec<AgentEntry> {
    let repo = match systemprompt_agent::repository::content::AgentRepository::new(ctx.db_pool()) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let agents = match repo.list_enabled().await {
        Ok(rs) => rs,
        Err(_) => return Vec::new(),
    };
    let mut out: Vec<AgentEntry> = agents
        .into_iter()
        .map(|a| AgentEntry {
            id: a.id.as_str().to_string(),
            name: a.name,
            display_name: a.display_name,
            description: a.description,
            version: a.version,
            endpoint: a.endpoint,
            enabled: a.enabled,
            is_default: a.is_default,
            is_primary: a.is_primary,
            provider: a.provider,
            model: a.model,
            mcp_servers: a.mcp_servers,
            skills: a.skills,
            tags: a.tags,
            card: a.card_json,
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
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

fn revocations() -> Vec<String> {
    Vec::new()
}

fn short_hash(
    plugins: &[PluginEntry],
    mcp: &[ManagedMcpServer],
    skills: &[SkillEntry],
    agents: &[AgentEntry],
) -> String {
    let mut h = Sha256::new();
    if let Ok(s) = serde_json::to_string(plugins) {
        h.update(s.as_bytes());
    }
    h.update(b"|");
    if let Ok(s) = serde_json::to_string(mcp) {
        h.update(s.as_bytes());
    }
    h.update(b"|");
    if let Ok(s) = serde_json::to_string(skills) {
        h.update(s.as_bytes());
    }
    h.update(b"|");
    if let Ok(s) = serde_json::to_string(agents) {
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
