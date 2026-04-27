use crate::config;
use crate::http::GatewayClient;
use crate::manifest::{
    AgentEntry, ManagedMcpServer, PluginEntry, SignedManifest, SkillEntry, UserInfo,
};
use crate::output::diag;
use crate::paths::{self, OrgPluginsLocation};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

const WATCH_FLOOR_SECS: u64 = 60;

pub struct SyncOptions {
    pub watch: bool,
    pub interval: Option<u64>,
    pub allow_unsigned: bool,
}

#[derive(Debug, Clone)]
pub struct SyncSummary {
    pub identity: String,
    pub manifest_version: String,
    pub plugin_count: usize,
    pub skill_count: usize,
    pub agent_count: usize,
    pub mcp_count: usize,
    pub installed: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
}

impl SyncSummary {
    pub fn one_line(&self) -> String {
        format!(
            "sync ok ({}): {} plugins ({} new, {} updated, {} removed), {} skills, {} agents, {} MCP — manifest {}",
            self.identity,
            self.plugin_count,
            self.installed.len(),
            self.updated.len(),
            self.removed.len(),
            self.skill_count,
            self.agent_count,
            self.mcp_count,
            self.manifest_version,
        )
    }
}

#[derive(Debug, Clone)]
pub enum SyncErrorKind {
    NoCredential,
    Network,
    SignatureFailed,
    PathUnresolvable,
    ApplyFailed,
}

#[derive(Debug, Clone)]
pub struct SyncError {
    pub kind: SyncErrorKind,
    pub message: String,
}

impl SyncError {
    fn exit_code(&self) -> ExitCode {
        match self.kind {
            SyncErrorKind::NoCredential => ExitCode::from(5),
            SyncErrorKind::Network => ExitCode::from(3),
            SyncErrorKind::SignatureFailed => ExitCode::from(4),
            SyncErrorKind::PathUnresolvable => ExitCode::from(1),
            SyncErrorKind::ApplyFailed => ExitCode::from(1),
        }
    }
}

pub fn sync(opts: SyncOptions) -> ExitCode {
    if !opts.watch {
        return run_once_cli(opts.allow_unsigned);
    }

    let interval = opts
        .interval
        .unwrap_or(1800)
        .max(WATCH_FLOOR_SECS);
    loop {
        let code = run_once_cli(opts.allow_unsigned);
        if code != ExitCode::SUCCESS {
            eprintln!(
                "sync: non-zero exit; retrying in {interval}s",
            );
        }
        std::thread::sleep(Duration::from_secs(interval));
    }
}

fn run_once_cli(allow_unsigned: bool) -> ExitCode {
    if allow_unsigned {
        eprintln!("warning: --allow-unsigned bypasses signature verification");
    }
    match run_once(allow_unsigned) {
        Ok(summary) => {
            println!("{}", summary.one_line());
            ExitCode::SUCCESS
        },
        Err(err) => {
            diag(&match err.kind {
                SyncErrorKind::NoCredential => err.message.clone(),
                SyncErrorKind::Network => err.message.clone(),
                SyncErrorKind::SignatureFailed => {
                    format!("manifest signature verification failed: {}", err.message)
                },
                SyncErrorKind::PathUnresolvable => err.message.clone(),
                SyncErrorKind::ApplyFailed => format!("sync apply failed: {}", err.message),
            });
            err.exit_code()
        },
    }
}

pub fn run_once(allow_unsigned: bool) -> Result<SyncSummary, SyncError> {
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);

    let bearer = match crate::cache::read_valid() {
        Some(out) => out.token,
        None => match fetch_fresh_token() {
            Some(t) => t,
            None => {
                return Err(SyncError {
                    kind: SyncErrorKind::NoCredential,
                    message:
                        "no valid credential available; run `systemprompt-cowork login` first"
                            .to_string(),
                });
            },
        },
    };

    let client = GatewayClient::new(gateway.clone());
    let manifest = client.fetch_manifest(&bearer).map_err(|e| SyncError {
        kind: SyncErrorKind::Network,
        message: e,
    })?;

    if !allow_unsigned {
        let pubkey = match config::pinned_pubkey() {
            Some(k) => k,
            None => match client.fetch_pubkey() {
                Ok(k) => {
                    let _ = config::persist_pinned_pubkey(&k);
                    k
                },
                Err(e) => {
                    return Err(SyncError {
                        kind: SyncErrorKind::Network,
                        message: format!("no pinned pubkey and live fetch failed: {e}"),
                    });
                },
            },
        };
        if let Err(e) = manifest.verify(&pubkey) {
            return Err(SyncError {
                kind: SyncErrorKind::SignatureFailed,
                message: e,
            });
        }
    }

    let location = paths::org_plugins_effective().ok_or_else(|| SyncError {
        kind: SyncErrorKind::PathUnresolvable,
        message: "org-plugins directory not resolvable".to_string(),
    })?;

    let report = apply_manifest(&client, &bearer, &manifest, &location).map_err(|e| {
        SyncError {
            kind: SyncErrorKind::ApplyFailed,
            message: e,
        }
    })?;

    let last_sync = paths::metadata_dir(&location.path).join(paths::LAST_SYNC_SENTINEL);
    let _ = fs::create_dir_all(paths::metadata_dir(&location.path));
    let _ = fs::write(
        &last_sync,
        serde_json::to_vec_pretty(&serde_json::json!({
            "synced_at": current_iso8601(),
            "manifest_version": manifest.manifest_version,
            "installed_plugins": report.installed,
            "updated_plugins": report.updated,
            "removed_plugins": report.removed,
            "mcp_server_count": manifest.managed_mcp_servers.len(),
            "skill_count": manifest.skills.len(),
            "agent_count": manifest.agents.len(),
            "user": manifest.user.as_ref().map(|u| &u.email),
        }))
        .unwrap_or_default(),
    );
    let identity = manifest
        .user
        .as_ref()
        .map(|u| u.email.clone())
        .unwrap_or_else(|| manifest.user_id.clone());

    Ok(SyncSummary {
        identity,
        manifest_version: manifest.manifest_version.clone(),
        plugin_count: manifest.plugins.len(),
        skill_count: manifest.skills.len(),
        agent_count: manifest.agents.len(),
        mcp_count: manifest.managed_mcp_servers.len(),
        installed: report.installed,
        updated: report.updated,
        removed: report.removed,
    })
}

struct ApplyReport {
    installed: Vec<String>,
    updated: Vec<String>,
    removed: Vec<String>,
}

fn apply_manifest(
    client: &GatewayClient,
    bearer: &str,
    manifest: &SignedManifest,
    location: &OrgPluginsLocation,
) -> Result<ApplyReport, String> {
    let root = &location.path;
    fs::create_dir_all(root).map_err(|e| format!("create {}: {e}", root.display()))?;
    let meta_dir = paths::metadata_dir(root);
    fs::create_dir_all(&meta_dir).map_err(|e| format!("create metadata dir: {e}"))?;
    let staging_root = paths::staging_dir(root);
    let _ = fs::remove_dir_all(&staging_root);
    fs::create_dir_all(&staging_root).map_err(|e| format!("create staging: {e}"))?;

    let mut installed = Vec::new();
    let mut updated = Vec::new();
    let expected_ids: HashSet<&str> =
        manifest.plugins.iter().map(|p| p.id.as_str()).collect();

    for plugin in &manifest.plugins {
        if !safe_plugin_id(&plugin.id) {
            return Err(format!("manifest contained unsafe plugin id: {}", plugin.id));
        }
        let target = root.join(&plugin.id);
        let current_hash = target.is_dir().then(|| directory_hash(&target).ok()).flatten();
        if current_hash.as_deref() == Some(plugin.sha256.as_str()) {
            continue;
        }

        let stage = staging_root.join(&plugin.id);
        fetch_plugin_into_staging(client, bearer, plugin, &stage)?;

        let staged_hash = directory_hash(&stage)
            .map_err(|e| format!("hash staged {}: {e}", plugin.id))?;
        if staged_hash != plugin.sha256 {
            return Err(format!(
                "plugin {} hash mismatch (expected {}, got {})",
                plugin.id, plugin.sha256, staged_hash
            ));
        }

        let was_present = target.exists();
        if was_present {
            fs::remove_dir_all(&target).map_err(|e| format!("remove old {}: {e}", plugin.id))?;
        }
        fs::rename(&stage, &target).map_err(|e| format!("rename stage→target for {}: {e}", plugin.id))?;

        if was_present {
            updated.push(plugin.id.clone());
        } else {
            installed.push(plugin.id.clone());
        }
    }

    let mut removed = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name_str) = name.to_str() else { continue };
            if name_str.starts_with('.') {
                continue;
            }
            if !expected_ids.contains(name_str) && entry.path().is_dir() {
                if let Err(e) = fs::remove_dir_all(entry.path()) {
                    return Err(format!("remove stale {name_str}: {e}"));
                }
                removed.push(name_str.to_string());
            }
        }
    }

    let _ = fs::remove_dir_all(&staging_root);

    write_managed_mcp_fragment(&meta_dir, &manifest.managed_mcp_servers)?;
    write_skills(&meta_dir, &manifest.skills)?;
    write_agents(&meta_dir, &manifest.agents)?;
    write_user(&meta_dir, manifest.user.as_ref())?;

    Ok(ApplyReport {
        installed,
        updated,
        removed,
    })
}

fn write_skills(meta_dir: &Path, skills: &[SkillEntry]) -> Result<(), String> {
    let dir = meta_dir.join(paths::SKILLS_DIR);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("clear skills dir: {e}"))?;
    }
    fs::create_dir_all(&dir).map_err(|e| format!("create skills dir: {e}"))?;
    let index: Vec<serde_json::Value> = skills
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "name": s.name,
                "description": s.description,
                "file_path": s.file_path,
                "tags": s.tags,
                "sha256": s.sha256,
            })
        })
        .collect();
    let index_path = dir.join("index.json");
    fs::write(
        &index_path,
        serde_json::to_vec_pretty(&index).unwrap_or_default(),
    )
    .map_err(|e| format!("write {}: {e}", index_path.display()))?;
    for skill in skills {
        if !safe_id_segment(&skill.id) {
            return Err(format!("manifest contained unsafe skill id: {}", skill.id));
        }
        let skill_dir = dir.join(&skill.id);
        fs::create_dir_all(&skill_dir).map_err(|e| format!("create {}: {e}", skill_dir.display()))?;
        let meta = serde_json::json!({
            "id": skill.id,
            "name": skill.name,
            "description": skill.description,
            "file_path": skill.file_path,
            "tags": skill.tags,
            "sha256": skill.sha256,
        });
        fs::write(
            skill_dir.join("metadata.json"),
            serde_json::to_vec_pretty(&meta).unwrap_or_default(),
        )
        .map_err(|e| format!("write skill metadata for {}: {e}", skill.id))?;
        fs::write(skill_dir.join("SKILL.md"), &skill.instructions)
            .map_err(|e| format!("write SKILL.md for {}: {e}", skill.id))?;
    }
    Ok(())
}

fn write_agents(meta_dir: &Path, agents: &[AgentEntry]) -> Result<(), String> {
    let dir = meta_dir.join(paths::AGENTS_DIR);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("clear agents dir: {e}"))?;
    }
    fs::create_dir_all(&dir).map_err(|e| format!("create agents dir: {e}"))?;
    let index: Vec<serde_json::Value> = agents
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "name": a.name,
                "display_name": a.display_name,
                "version": a.version,
                "endpoint": a.endpoint,
                "is_default": a.is_default,
                "is_primary": a.is_primary,
            })
        })
        .collect();
    fs::write(
        dir.join("index.json"),
        serde_json::to_vec_pretty(&index).unwrap_or_default(),
    )
    .map_err(|e| format!("write agents index: {e}"))?;
    for agent in agents {
        if !safe_id_segment(&agent.name) {
            return Err(format!("manifest contained unsafe agent name: {}", agent.name));
        }
        let path = dir.join(format!("{}.json", agent.name));
        fs::write(&path, serde_json::to_vec_pretty(agent).unwrap_or_default())
            .map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(())
}

fn write_user(meta_dir: &Path, user: Option<&UserInfo>) -> Result<(), String> {
    let path = meta_dir.join(paths::USER_FRAGMENT);
    let bytes = match user {
        Some(u) => serde_json::to_vec_pretty(u)
            .map_err(|e| format!("serialize user: {e}"))?,
        None => b"null".to_vec(),
    };
    fs::write(&path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}

fn safe_id_segment(s: &str) -> bool {
    !s.is_empty()
        && !s.contains("..")
        && !s.contains('/')
        && !s.contains('\\')
        && !s.starts_with('.')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

fn fetch_plugin_into_staging(
    client: &GatewayClient,
    bearer: &str,
    plugin: &PluginEntry,
    stage: &Path,
) -> Result<(), String> {
    fs::create_dir_all(stage).map_err(|e| format!("create stage {}: {e}", stage.display()))?;
    for file in &plugin.files {
        if file.path.contains("..") || file.path.starts_with('/') || file.path.starts_with('\\') {
            return Err(format!("unsafe path in manifest: {}", file.path));
        }
        let out = stage.join(normalise_relative(&file.path));
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("create parent {}: {e}", parent.display()))?;
        }
        let bytes = client.fetch_plugin_file(bearer, &plugin.id, &file.path)?;
        let actual = sha256_hex(&bytes);
        if actual != file.sha256 {
            return Err(format!(
                "file {}/{} hash mismatch (expected {}, got {})",
                plugin.id, file.path, file.sha256, actual
            ));
        }
        fs::write(&out, &bytes).map_err(|e| format!("write {}: {e}", out.display()))?;
    }
    Ok(())
}

fn write_managed_mcp_fragment(
    meta_dir: &Path,
    servers: &[ManagedMcpServer],
) -> Result<(), String> {
    let out = meta_dir.join(paths::MANAGED_MCP_FRAGMENT);
    let bytes = serde_json::to_vec_pretty(servers)
        .map_err(|e| format!("serialize managed-mcp: {e}"))?;
    fs::write(&out, bytes).map_err(|e| format!("write {}: {e}", out.display()))
}

fn safe_plugin_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains("..")
        && !id.contains('/')
        && !id.contains('\\')
        && !id.starts_with('.')
}

fn normalise_relative(p: &str) -> PathBuf {
    PathBuf::from(p.replace('\\', "/"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex_encode(&h.finalize())
}

fn directory_hash(root: &Path) -> std::io::Result<String> {
    let mut entries: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    collect_files(root, root, &mut entries)?;
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    for (rel, bytes) in &entries {
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        hasher.update(bytes);
        hasher.update(b"\0");
    }
    Ok(hex_encode(&hasher.finalize()))
}

fn collect_files(
    base: &Path,
    dir: &Path,
    out: &mut Vec<(PathBuf, Vec<u8>)>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            collect_files(base, &path, out)?;
        } else if ft.is_file() {
            let bytes = fs::read(&path)?;
            let rel = path.strip_prefix(base).unwrap_or(&path).to_path_buf();
            out.push((rel, bytes));
        }
    }
    Ok(())
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

fn fetch_fresh_token() -> Option<String> {
    use crate::providers::{AuthError, AuthProvider};
    let cfg = config::load();
    let chain: Vec<Box<dyn AuthProvider>> = vec![
        Box::new(crate::providers::mtls::MtlsProvider::new(&cfg)),
        Box::new(crate::providers::session::SessionProvider::new(&cfg)),
        Box::new(crate::providers::pat::PatProvider::new(&cfg)),
    ];
    for p in &chain {
        match p.authenticate() {
            Ok(out) => {
                let _ = crate::cache::write(&out);
                return Some(out.token);
            },
            Err(AuthError::NotConfigured) => continue,
            Err(AuthError::Failed(msg)) => {
                diag(&format!("{}: {msg}", p.name()));
            },
        }
    }
    None
}

fn current_iso8601() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".into())
}
