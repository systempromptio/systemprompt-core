use crate::config;
use crate::http::GatewayClient;
use crate::manifest::{ManagedMcpServer, PluginEntry, SignedManifest};
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

pub fn sync(opts: SyncOptions) -> ExitCode {
    if !opts.watch {
        return run_once(opts.allow_unsigned);
    }

    let interval = opts
        .interval
        .unwrap_or(1800)
        .max(WATCH_FLOOR_SECS);
    loop {
        let code = run_once(opts.allow_unsigned);
        if code != ExitCode::SUCCESS {
            eprintln!(
                "sync: non-zero exit; retrying in {interval}s",
            );
        }
        std::thread::sleep(Duration::from_secs(interval));
    }
}

fn run_once(allow_unsigned: bool) -> ExitCode {
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);

    let bearer = match crate::cache::read_valid() {
        Some(out) => out.token,
        None => match fetch_fresh_token() {
            Some(t) => t,
            None => {
                diag("no valid credential available; run `systemprompt-cowork login` first");
                return ExitCode::from(5);
            },
        },
    };

    let client = GatewayClient::new(gateway.clone());
    let manifest = match client.fetch_manifest(&bearer) {
        Ok(m) => m,
        Err(e) => {
            diag(&e);
            return ExitCode::from(3);
        },
    };

    if !allow_unsigned {
        let pubkey = match config::pinned_pubkey() {
            Some(k) => k,
            None => match client.fetch_pubkey() {
                Ok(k) => {
                    let _ = config::persist_pinned_pubkey(&k);
                    k
                },
                Err(e) => {
                    diag(&format!("no pinned pubkey and live fetch failed: {e}"));
                    return ExitCode::from(3);
                },
            },
        };
        if let Err(e) = manifest.verify(&pubkey) {
            diag(&format!("manifest signature verification failed: {e}"));
            return ExitCode::from(4);
        }
    } else {
        eprintln!("warning: --allow-unsigned bypasses signature verification");
    }

    let location = match paths::org_plugins_effective() {
        Some(l) => l,
        None => {
            diag("org-plugins directory not resolvable");
            return ExitCode::from(1);
        },
    };

    match apply_manifest(&client, &bearer, &manifest, &location) {
        Ok(report) => {
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
                }))
                .unwrap_or_default(),
            );
            println!(
                "sync ok: {} installed, {} updated, {} removed ({} MCP servers, manifest {})",
                report.installed.len(),
                report.updated.len(),
                report.removed.len(),
                manifest.managed_mcp_servers.len(),
                manifest.manifest_version,
            );
            ExitCode::SUCCESS
        },
        Err(e) => {
            diag(&format!("sync apply failed: {e}"));
            ExitCode::from(1)
        },
    }
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

    Ok(ApplyReport {
        installed,
        updated,
        removed,
    })
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
