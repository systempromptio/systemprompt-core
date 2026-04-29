use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const PAT_FILENAME: &str = "systemprompt-cowork.pat";
const CONFIG_FILENAME: &str = "systemprompt-cowork.toml";
const DEFAULT_GATEWAY_URL: &str = "http://localhost:8080";
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

pub struct PathLayout {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub pat_file: PathBuf,
}

pub fn resolve_paths() -> Result<PathLayout, SetupError> {
    let base = dirs::config_dir().ok_or_else(|| {
        SetupError::Path("no OS config directory available on this platform".to_string())
    })?;
    let config_dir = base.join("systemprompt");
    let config_file = config_dir.join(CONFIG_FILENAME);
    let pat_file = config_dir.join(PAT_FILENAME);
    Ok(PathLayout {
        config_dir,
        config_file,
        pat_file,
    })
}

pub fn login(token: &str, gateway_url: Option<&str>) -> Result<PathLayout, SetupError> {
    validate_token(token)?;
    let paths = resolve_paths()?;
    ensure_dir(&paths.config_dir)?;
    write_pat_file(&paths.pat_file, token)?;
    write_config_file(&paths.config_file, &paths.pat_file, gateway_url)?;
    Ok(paths)
}

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

pub fn logout() -> Result<PathLayout, SetupError> {
    let paths = resolve_paths()?;
    remove_if_exists(&paths.pat_file)?;
    if let Err(e) = crate::cache::clear() {
        return Err(SetupError::Io(format!("clear token cache: {e}")));
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

pub fn clean() -> Result<CleanReport, SetupError> {
    let paths = resolve_paths()?;
    let pat_removed = paths.pat_file.exists();
    remove_if_exists(&paths.pat_file)?;
    let config_removed = paths.config_file.exists();
    remove_if_exists(&paths.config_file)?;
    if let Err(e) = crate::cache::clear() {
        return Err(SetupError::Io(format!("clear token cache: {e}")));
    }
    Ok(CleanReport {
        paths,
        pat_removed,
        config_removed,
    })
}

pub struct CleanReport {
    pub paths: PathLayout,
    pub pat_removed: bool,
    pub config_removed: bool,
}

pub fn status() -> Result<StatusReport, SetupError> {
    let paths = resolve_paths()?;
    let config_present = paths.config_file.exists();
    let pat_present = paths.pat_file.exists();
    Ok(StatusReport {
        paths,
        config_present,
        pat_present,
    })
}

pub struct StatusReport {
    pub paths: PathLayout,
    pub config_present: bool,
    pub pat_present: bool,
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

fn write_config_file(
    path: &Path,
    pat_file: &Path,
    gateway_url_override: Option<&str>,
) -> Result<(), SetupError> {
    let existing_gateway = fs::read_to_string(path)
        .ok()
        .and_then(|s| -> Option<String> {
            for line in s.lines() {
                let t = line.trim();
                if let Some(rest) = t.strip_prefix("gateway_url") {
                    let rest = rest.trim().trim_start_matches('=').trim();
                    let rest = rest.trim_matches('"').trim_matches('\'');
                    if !rest.is_empty() {
                        return Some(rest.to_string());
                    }
                }
            }
            None
        });
    let gateway = gateway_url_override
        .map(str::to_string)
        .or(existing_gateway)
        .unwrap_or_else(|| DEFAULT_GATEWAY_URL.to_string());

    let pat_path_str = pat_file.to_string_lossy().replace('\\', "\\\\");
    let contents = format!(
        "# Written by `systemprompt-cowork login`. Edit gateway_url if you move the \
         server.\ngateway_url = \"{gateway}\"\n\n[pat]\nfile = \"{pat_path_str}\"\n"
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
            .unwrap_or("systemprompt-cowork")
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
