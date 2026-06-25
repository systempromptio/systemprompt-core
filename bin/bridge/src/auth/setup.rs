use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const PAT_PREFIX: &str = "sp-live-";

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("{0}")]
    Token(String),
    #[error("{0}")]
    Path(String),
    #[error("{0}")]
    Io(String),
}

#[derive(Debug)]
pub struct PathLayout {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub pat_file: PathBuf,
}

pub fn resolve_paths() -> Result<PathLayout, SetupError> {
    let base = dirs::config_dir().ok_or_else(|| {
        SetupError::Path("no OS config directory available on this platform".to_owned())
    })?;
    let brand = crate::brand::brand();
    let config_dir = base.join(brand.config_dir);
    let config_file = config_dir.join(brand.config_file);
    let pat_file = config_dir.join(brand.pat_file);
    Ok(PathLayout {
        config_dir,
        config_file,
        pat_file,
    })
}

#[tracing::instrument(level = "debug", skip(token), fields(has_gateway = gateway_url.is_some()))]
pub fn login(token: &str, gateway_url: Option<&str>) -> Result<PathLayout, SetupError> {
    validate_token(token)?;
    let paths = resolve_paths()?;
    ensure_dir(&paths.config_dir)?;
    write_pat_file(&paths.pat_file, token)?;
    write_config_file(&paths.config_file, &paths.pat_file, gateway_url)?;
    tracing::info!(config_file = %paths.config_file.display(), "login: PAT and config written");
    Ok(paths)
}

#[tracing::instrument(level = "debug")]
pub fn set_gateway_url(gateway_url: &str) -> Result<PathLayout, SetupError> {
    let trimmed = gateway_url.trim();
    if trimmed.is_empty() {
        return Err(SetupError::Path("gateway_url is empty".into()));
    }
    let paths = resolve_paths()?;
    ensure_dir(&paths.config_dir)?;
    write_config_file(&paths.config_file, &paths.pat_file, Some(trimmed))?;
    Ok(paths)
}

#[tracing::instrument(level = "debug")]
pub fn logout() -> Result<PathLayout, SetupError> {
    let paths = resolve_paths()?;
    remove_if_exists(&paths.pat_file)?;
    remove_managed_mcp_fragment()?;
    if let Err(e) = crate::auth::cache::clear() {
        return Err(SetupError::Io(format!("clear token cache: {e}")));
    }
    if let Err(e) = crate::auth::plugin_oauth::delete_creds() {
        return Err(SetupError::Io(format!("clear oauth client creds: {e}")));
    }
    if paths.config_file.exists() {
        match fs::read_to_string(&paths.config_file) {
            Ok(existing) => {
                let stripped = strip_pat_section(&existing);
                if stripped.trim().is_empty() {
                    remove_if_exists(&paths.config_file)?;
                } else {
                    atomic_write(&paths.config_file, stripped.as_bytes(), false)?;
                }
            },
            Err(e) => return Err(SetupError::Io(format!("read config: {e}"))),
        }
    }
    Ok(paths)
}

#[tracing::instrument(level = "debug")]
pub fn clean() -> Result<CleanReport, SetupError> {
    let paths = resolve_paths()?;
    let pat_removed = paths.pat_file.exists();
    remove_if_exists(&paths.pat_file)?;
    let config_removed = paths.config_file.exists();
    remove_if_exists(&paths.config_file)?;
    remove_managed_mcp_fragment()?;
    if let Err(e) = crate::auth::cache::clear() {
        return Err(SetupError::Io(format!("clear token cache: {e}")));
    }
    let oauth_creds_removed = crate::auth::plugin_oauth::creds_path().is_some_and(|p| p.exists());
    if let Err(e) = crate::auth::plugin_oauth::delete_creds() {
        return Err(SetupError::Io(format!("clear oauth client creds: {e}")));
    }
    Ok(CleanReport {
        paths,
        pat_removed,
        config_removed,
        oauth_creds_removed,
    })
}

#[derive(Debug)]
pub struct CleanReport {
    pub paths: PathLayout,
    pub pat_removed: bool,
    pub config_removed: bool,
    pub oauth_creds_removed: bool,
}

pub fn status() -> Result<StatusReport, SetupError> {
    let paths = resolve_paths()?;
    let config_present = paths.config_file.exists();
    let pat_present = paths.pat_file.exists();
    let oauth_creds_path = crate::auth::plugin_oauth::creds_path();
    let oauth_creds_present = oauth_creds_path.as_ref().is_some_and(|p| p.exists());
    Ok(StatusReport {
        paths,
        config_present,
        pat_present,
        oauth_creds_path,
        oauth_creds_present,
    })
}

#[derive(Debug)]
pub struct StatusReport {
    pub paths: PathLayout,
    pub config_present: bool,
    pub pat_present: bool,
    pub oauth_creds_path: Option<PathBuf>,
    pub oauth_creds_present: bool,
}

fn validate_token(token: &str) -> Result<(), SetupError> {
    let trimmed = token.trim();
    if !trimmed.starts_with(PAT_PREFIX) {
        return Err(SetupError::Token(format!(
            "token must start with `{PAT_PREFIX}`"
        )));
    }
    if !trimmed.contains('.') {
        return Err(SetupError::Token(
            "token must contain a `.` separator (sp-live-<prefix>.<secret>)".into(),
        ));
    }
    if trimmed.len() < 40 {
        return Err(SetupError::Token(
            "token looks too short — did the copy get truncated?".into(),
        ));
    }
    Ok(())
}

fn ensure_dir(dir: &Path) -> Result<(), SetupError> {
    fs::create_dir_all(dir)
        .map_err(|e| SetupError::Io(format!("create config dir {}: {e}", dir.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dir)
            .map_err(|e| SetupError::Io(format!("stat dir: {e}")))?
            .permissions();
        perms.set_mode(0o700);
        fs::set_permissions(dir, perms).map_err(|e| SetupError::Io(format!("chmod dir: {e}")))?;
    }
    Ok(())
}

fn write_pat_file(path: &Path, token: &str) -> Result<(), SetupError> {
    atomic_write(path, token.trim().as_bytes(), true)
}

fn read_existing_gateway(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    for line in contents.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("gateway_url") {
            let rest = rest.trim().trim_start_matches('=').trim();
            let rest = rest.trim_matches('"').trim_matches('\'');
            if !rest.is_empty() {
                return Some(rest.to_owned());
            }
        }
    }
    None
}

fn resolve_gateway(path: &Path, gateway_url_override: Option<&str>) -> String {
    gateway_url_override
        .map(str::to_owned)
        .or_else(|| read_existing_gateway(path))
        .unwrap_or_else(|| crate::brand::brand().default_gateway_url.to_owned())
}

/// Writes no `[pat]` section — the session flow stores no long-lived secret;
/// the proxy mints short-lived JWTs from the cached device-link credential.
pub fn session_setup(gateway_url: Option<&str>) -> Result<PathLayout, SetupError> {
    let paths = resolve_paths()?;
    ensure_dir(&paths.config_dir)?;
    let gateway = resolve_gateway(&paths.config_file, gateway_url);
    let contents = format!(
        "# Written by `{bin}` sign-in. Edit gateway_url if you move the \
         server.\ngateway_url = \"{gateway}\"\n\n[session]\nenabled = true\n",
        bin = crate::brand::brand().binary_name,
    );
    atomic_write(&paths.config_file, contents.as_bytes(), false)?;
    tracing::info!(config_file = %paths.config_file.display(), "session setup: config written");
    Ok(paths)
}

fn write_config_file(
    path: &Path,
    pat_file: &Path,
    gateway_url_override: Option<&str>,
) -> Result<(), SetupError> {
    let gateway = resolve_gateway(path, gateway_url_override);

    let pat_path_str = pat_file.to_string_lossy().replace('\\', "\\\\");
    let contents = format!(
        "# Written by `{bin} login`. Edit gateway_url if you move the \
         server.\ngateway_url = \"{gateway}\"\n\n[pat]\nfile = \"{pat_path_str}\"\n",
        bin = crate::brand::brand().binary_name,
    );
    atomic_write(path, contents.as_bytes(), false)
}

fn atomic_write(target: &Path, bytes: &[u8], secret: bool) -> Result<(), SetupError> {
    let parent = target
        .parent()
        .ok_or_else(|| SetupError::Path(format!("no parent dir for {}", target.display())))?;
    let tmp = parent.join(format!(
        ".{}.tmp",
        target
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| crate::brand::brand().binary_name)
    ));
    {
        let mut f = create_restricted(&tmp, secret)?;
        f.write_all(bytes)
            .map_err(|e| SetupError::Io(format!("write {}: {e}", tmp.display())))?;
        f.sync_all()
            .map_err(|e| SetupError::Io(format!("fsync {}: {e}", tmp.display())))?;
    }
    fs::rename(&tmp, target).map_err(|e| {
        SetupError::Io(format!(
            "rename {} -> {}: {e}",
            tmp.display(),
            target.display()
        ))
    })?;
    Ok(())
}

#[cfg(unix)]
fn create_restricted(path: &Path, secret: bool) -> Result<File, SetupError> {
    use std::os::unix::fs::OpenOptionsExt;
    let mode = if secret { 0o600 } else { 0o644 };
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(path)
        .map_err(|e| SetupError::Io(format!("open {}: {e}", path.display())))
}

#[cfg(not(unix))]
fn create_restricted(path: &Path, _secret: bool) -> Result<File, SetupError> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|e| SetupError::Io(format!("open {}: {e}", path.display())))
}

fn remove_if_exists(path: &Path) -> Result<(), SetupError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(SetupError::Io(format!("remove {}: {e}", path.display()))),
    }
}

fn remove_managed_mcp_fragment() -> Result<(), SetupError> {
    let Some(meta_dir) = crate::config::paths::bridge_metadata_dir() else {
        return Ok(());
    };
    remove_if_exists(&meta_dir.join(crate::config::paths::MCP_SERVERS_FRAGMENT))
}

fn strip_pat_section(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_pat = false;
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_pat = trimmed == "[pat]";
            if in_pat {
                continue;
            }
        }
        if in_pat {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}
