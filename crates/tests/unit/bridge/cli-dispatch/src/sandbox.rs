use std::path::PathBuf;
use tempfile::TempDir;

pub struct Sandbox {
    pub home: TempDir,
    pub config: TempDir,
    pub data: TempDir,
    pub state: TempDir,
}

impl Sandbox {
    pub fn new() -> Self {
        Self {
            home: TempDir::new().expect("home tempdir"),
            config: TempDir::new().expect("config tempdir"),
            data: TempDir::new().expect("data tempdir"),
            state: TempDir::new().expect("state tempdir"),
        }
    }

    pub fn org_plugins(&self) -> PathBuf {
        self.data.path().join("Claude").join("org-plugins")
    }

    pub fn metadata(&self) -> PathBuf {
        self.state
            .path()
            .join("systemprompt-bridge")
            .join("metadata")
    }

    pub fn vars(&self) -> Vec<(&'static str, Option<String>)> {
        vec![
            ("HOME", p(self.home.path())),
            ("XDG_CONFIG_HOME", p(self.config.path())),
            ("XDG_DATA_HOME", p(self.data.path())),
            ("XDG_STATE_HOME", p(self.state.path())),
            ("XDG_CACHE_HOME", p(self.home.path())),
            ("SP_BRIDGE_CONFIG", None),
            ("SP_BRIDGE_PAT", None),
            ("SP_BRIDGE_GATEWAY_URL", None),
            ("SUDO_USER", None),
        ]
    }

    pub fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        temp_env::with_vars(self.vars(), f)
    }
}

fn p(path: &std::path::Path) -> Option<String> {
    Some(path.to_str().expect("utf-8 tempdir path").to_owned())
}

pub fn argv(parts: &[&str]) -> Vec<String> {
    let mut v = vec!["systemprompt-bridge".to_owned()];
    v.extend(parts.iter().map(|s| (*s).to_owned()));
    v
}
