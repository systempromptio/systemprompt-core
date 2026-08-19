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

    // Points the system org-plugins root inside the sandbox so an in-process
    // `install` can never provision the host's real one (on CI runners /opt is
    // writable) and poison sibling suites. It is the same path the assertions
    // read: macOS takes the system scope unconditionally and never probes
    // writability, so an unwritable decoy there would resolve to a directory
    // no test looks at rather than falling back the way Linux does. Which
    // scope wins is asserted directly in the `paths` suite.
    fn system_org_plugins(&self) -> Option<String> {
        p(&self.org_plugins())
    }

    pub fn vars(&self) -> Vec<(&'static str, Option<String>)> {
        vec![
            ("HOME", p(self.home.path())),
            ("SP_BRIDGE_ORG_PLUGINS_SYSTEM", self.system_org_plugins()),
            ("XDG_CONFIG_HOME", p(self.config.path())),
            ("XDG_DATA_HOME", p(self.data.path())),
            ("XDG_STATE_HOME", p(self.state.path())),
            ("XDG_CACHE_HOME", p(self.home.path())),
            ("SP_BRIDGE_CONFIG", None),
            ("SP_BRIDGE_PAT", None),
            ("SUDO_USER", None),
        ]
    }

    pub fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        temp_env::with_vars(self.vars(), f)
    }

    // The gateway env override was removed from the bridge, so tests point it
    // at a mock server the same way an operator would: through the config toml.
    pub fn write_gateway(&self, url: &str) {
        let dir = self.config.path().join("systemprompt");
        std::fs::create_dir_all(&dir).expect("config dir");
        std::fs::write(
            dir.join("systemprompt-bridge.toml"),
            format!("gateway_url = \"{url}\"\n"),
        )
        .expect("write gateway config");
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
