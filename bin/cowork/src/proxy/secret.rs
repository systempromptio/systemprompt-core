use std::fs;
use std::io::Read;
use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::ids::{LoopbackSecret, ProxySecret};

const LOOPBACK_FILENAME: &str = "cowork-loopback.key";

pub fn secret_path() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    Some(base.join("systemprompt").join(LOOPBACK_FILENAME))
}

pub fn load_or_mint() -> std::io::Result<String> {
    load_or_mint_typed().map(LoopbackSecret::into_inner)
}

pub fn load_or_mint_typed() -> std::io::Result<LoopbackSecret> {
    let path = secret_path()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no config dir"))?;
    if let Ok(bytes) = fs::read(&path) {
        let s = String::from_utf8_lossy(&bytes).trim().to_string();
        if !s.is_empty() {
            return Ok(LoopbackSecret::new(s));
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut buf = [0u8; 32];
    let mut urandom = fs::File::open("/dev/urandom")?;
    urandom.read_exact(&mut buf)?;
    let secret = URL_SAFE_NO_PAD.encode(buf);
    fs::write(&path, secret.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(LoopbackSecret::new(secret))
}

pub fn verify(presented: &str, expected: &ProxySecret) -> bool {
    constant_time_eq(presented.as_bytes(), expected.as_str().as_bytes())
}

fn constant_time_eq(presented: &[u8], expected: &[u8]) -> bool {
    if presented.len() != expected.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in presented.iter().zip(expected) {
        diff |= a ^ b;
    }
    diff == 0
}
