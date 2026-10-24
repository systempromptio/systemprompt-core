//! The `opencode run` command line and environment.
//!
//! The run is `opencode run --standalone --format json --auto`, with the
//! prompt after `--` and the workspace as the spawn cwd (`run` takes no
//! directory flag). `HOME` and every `XDG_*_HOME` point into the run home so
//! no user or machine config tier other than the managed one is reachable;
//! the `OPENCODE_*` switches are the ones `@opencode/cli` 2.1.3 reads
//! (`packages/cli/src/server-process.ts`, `services/updater.ts`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use super::config::PROVIDER_ID;
use super::{PAT_ENV, PROGRAM};
use crate::credentials::RunCredentials;
use crate::host::HostInvocation;
use crate::layout::RunLayout;
use crate::request::RunRequest;

pub const DISABLE_SWITCHES: [&str; 4] = [
    "OPENCODE_CONFIG_PROJECT_DISABLE",
    "OPENCODE_DISABLE_AUTOUPDATE",
    "OPENCODE_DISABLE_MODELS_FETCH",
    "OPENCODE_DISABLE_CLAUDE_CODE",
];

/// Where the generated `OpenCode` files live inside a [`RunLayout`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenCodePaths {
    pub home: PathBuf,
    pub config_home: PathBuf,
    pub data_home: PathBuf,
    pub state_home: PathBuf,
    pub cache_home: PathBuf,
    pub config_dir: PathBuf,
    pub config_json: PathBuf,
    pub agents_md: PathBuf,
}

impl OpenCodePaths {
    #[must_use]
    pub fn new(layout: &RunLayout) -> Self {
        let home = layout.home.clone();
        let config_home = home.join(".config");
        let config_dir = config_home.join(PROGRAM);
        Self {
            data_home: home.join(".local/share"),
            state_home: home.join(".local/state"),
            cache_home: home.join(".cache"),
            config_json: config_dir.join("opencode.json"),
            agents_md: config_dir.join("AGENTS.md"),
            config_dir,
            config_home,
            home,
        }
    }
}

#[must_use]
pub fn build_args(request: &RunRequest) -> Vec<String> {
    let mut args: Vec<String> = ["run", "--standalone", "--format", "json", "--auto"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    if let Some(alias) = &request.model {
        args.extend(["-m".to_owned(), format!("{PROVIDER_ID}/{alias}")]);
    }
    if let Some(session) = &request.resume_session_id {
        args.extend(["--session".to_owned(), session.as_str().to_owned()]);
    }
    args.extend(["--".to_owned(), request.prompt.clone()]);
    args
}

#[must_use]
pub fn build_env(credentials: &RunCredentials, paths: &OpenCodePaths) -> Vec<(String, String)> {
    let mut env = vec![
        ("HOME".to_owned(), paths.home.display().to_string()),
        (
            "XDG_CONFIG_HOME".to_owned(),
            paths.config_home.display().to_string(),
        ),
        (
            "XDG_DATA_HOME".to_owned(),
            paths.data_home.display().to_string(),
        ),
        (
            "XDG_STATE_HOME".to_owned(),
            paths.state_home.display().to_string(),
        ),
        (
            "XDG_CACHE_HOME".to_owned(),
            paths.cache_home.display().to_string(),
        ),
        (PAT_ENV.to_owned(), credentials.pat.as_str().to_owned()),
        ("DO_NOT_TRACK".to_owned(), "1".to_owned()),
    ];
    env.extend(
        DISABLE_SWITCHES
            .iter()
            .map(|key| ((*key).to_owned(), "1".to_owned())),
    );
    env
}

#[must_use]
pub fn invocation(
    request: &RunRequest,
    credentials: &RunCredentials,
    paths: &OpenCodePaths,
) -> HostInvocation {
    HostInvocation {
        program: PROGRAM.to_owned(),
        args: build_args(request),
        env: build_env(credentials, paths),
        cwd: request.workspace.clone(),
        stdin: None,
    }
}

#[must_use]
pub fn agents_md(request: &RunRequest) -> Option<String> {
    let parts: Vec<&str> = [&request.persona, &request.system_prompt_append]
        .into_iter()
        .filter_map(|part| part.as_deref())
        .filter(|part| !part.trim().is_empty())
        .collect();
    (!parts.is_empty()).then(|| format!("{}\n", parts.join("\n\n")))
}
