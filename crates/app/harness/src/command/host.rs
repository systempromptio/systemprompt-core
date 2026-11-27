//! What the binary tells the headless entry about itself, and the directories
//! under its data root.
//!
//! The directory names are the ones the window uses, so a sign-in made by
//! either is the other's: the session store is `data/sessions.db` and the
//! accounts are `state/accounts.json`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fmt, io};

use systemprompt_device::secure::Sealer;
use systemprompt_engine::config::Paths;

use crate::extension::{HostInfo, SettingsAccess, SpExtensionRegistration};

const STORE_FILE: &str = "sessions.db";
const ACCOUNTS_FILE: &str = "accounts.json";

/// Every directory a headless command writes to, all under one root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRoot {
    pub root: PathBuf,
    pub data: PathBuf,
    pub config: PathBuf,
    pub cache: PathBuf,
    pub state: PathBuf,
    pub tmp: PathBuf,
    pub logs: PathBuf,
    pub home: PathBuf,
}

impl DataRoot {
    pub fn under(root: PathBuf, home: PathBuf) -> Self {
        Self {
            data: root.join("data"),
            config: root.join("config"),
            cache: root.join("cache"),
            state: root.join("state"),
            tmp: root.join("tmp"),
            logs: root.join("logs"),
            root,
            home,
        }
    }

    pub fn ensure(&self) -> io::Result<()> {
        for dir in [
            &self.data,
            &self.config,
            &self.cache,
            &self.state,
            &self.tmp,
            &self.logs,
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn engine_paths(&self) -> Paths {
        Paths {
            home: self.home.clone(),
            data: self.data.clone(),
            config: self.config.clone(),
            cache: self.cache.clone(),
            state: self.state.clone(),
            tmp: self.tmp.clone(),
        }
    }

    pub fn store_file(&self) -> PathBuf {
        self.data.join(STORE_FILE)
    }

    pub fn accounts_file(&self) -> PathBuf {
        self.state.join(ACCOUNTS_FILE)
    }
}

/// The build a headless command runs as.
///
/// `info` is the brand's product, display name and version; `project_dir` is
/// the per-project configuration directory name; `device_name` names this
/// computer in the user's token list; `data_root` is the default root, which
/// a command's `--data-root` replaces; `sealer` keeps the personal access
/// token for the next process; `registrations` is the composition's extension
/// list; `verdicts_enabled` is the verdict preference the window keeps in its
/// own settings.
#[derive(Clone)]
pub struct HeadlessHost {
    pub info: HostInfo,
    pub project_dir: String,
    pub device_name: String,
    pub data_root: PathBuf,
    pub home: PathBuf,
    pub documents: Option<PathBuf>,
    pub sealer: Arc<dyn Sealer>,
    pub settings: Arc<dyn SettingsAccess>,
    pub registrations: Vec<SpExtensionRegistration>,
    pub verdicts_enabled: bool,
    pub search_path: Option<OsString>,
    pub path_ext: Option<OsString>,
}

impl HeadlessHost {
    pub fn root(&self, chosen: Option<&Path>) -> DataRoot {
        let root = chosen.map_or_else(|| self.data_root.clone(), Path::to_path_buf);
        DataRoot::under(root, self.home.clone())
    }
}

impl fmt::Debug for HeadlessHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HeadlessHost")
            .field("info", &self.info)
            .field("project_dir", &self.project_dir)
            .field("device_name", &self.device_name)
            .field("data_root", &self.data_root)
            .field("registrations", &self.registrations)
            .field("verdicts_enabled", &self.verdicts_enabled)
            .finish_non_exhaustive()
    }
}
