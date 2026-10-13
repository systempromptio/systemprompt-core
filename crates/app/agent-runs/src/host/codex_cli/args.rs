//! The `codex exec` command line and environment.
//!
//! The prompt goes on stdin (`-`), because Codex otherwise blocks reading a
//! piped stdin and would append it to the prompt. A resumed run is
//! `codex exec resume <thread id> -`, which takes no `--cd`; the working
//! directory is the spawn cwd in both shapes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use super::{PAT_ENV, PROGRAM};
use crate::credentials::RunCredentials;
use crate::host::HostInvocation;
use crate::layout::RunLayout;
use crate::request::RunRequest;

/// Where the generated Codex files live inside a [`RunLayout`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexPaths {
    pub home: PathBuf,
    pub codex_home: PathBuf,
    pub config_toml: PathBuf,
    pub output_schema: PathBuf,
    pub last_message: PathBuf,
}

impl CodexPaths {
    #[must_use]
    pub fn new(layout: &RunLayout) -> Self {
        let codex_home = layout.home.join(".codex");
        Self {
            home: layout.home.clone(),
            config_toml: codex_home.join("config.toml"),
            codex_home,
            output_schema: layout.host.join("output-schema.json"),
            last_message: layout.logs.join("last-message.txt"),
        }
    }
}

#[must_use]
pub fn build_args(request: &RunRequest, paths: &CodexPaths) -> Vec<String> {
    let mut args = vec!["exec".to_owned()];
    if request.resume_session_id.is_some() {
        args.push("resume".to_owned());
    }
    args.extend(["--json".to_owned(), "--skip-git-repo-check".to_owned()]);
    if request.resume_session_id.is_none() {
        args.extend(["--cd".to_owned(), request.workspace.display().to_string()]);
    }
    if let Some(model) = &request.model {
        args.extend(["--model".to_owned(), model.clone()]);
    }
    if request.output_schema.is_some() {
        args.extend([
            "--output-schema".to_owned(),
            paths.output_schema.display().to_string(),
        ]);
    }
    args.extend([
        "--output-last-message".to_owned(),
        paths.last_message.display().to_string(),
    ]);
    if let Some(session) = &request.resume_session_id {
        args.push(session.as_str().to_owned());
    }
    args.push("-".to_owned());
    args
}

#[must_use]
pub fn build_env(credentials: &RunCredentials, paths: &CodexPaths) -> Vec<(String, String)> {
    vec![
        ("HOME".to_owned(), paths.home.display().to_string()),
        (
            "CODEX_HOME".to_owned(),
            paths.codex_home.display().to_string(),
        ),
        (PAT_ENV.to_owned(), credentials.pat.as_str().to_owned()),
    ]
}

#[must_use]
pub fn invocation(
    request: &RunRequest,
    credentials: &RunCredentials,
    paths: &CodexPaths,
) -> HostInvocation {
    HostInvocation {
        program: PROGRAM.to_owned(),
        args: build_args(request, paths),
        env: build_env(credentials, paths),
        cwd: request.workspace.clone(),
        stdin: Some(request.prompt.clone()),
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
