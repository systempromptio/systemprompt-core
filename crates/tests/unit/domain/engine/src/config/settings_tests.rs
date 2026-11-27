use std::path::{Path, PathBuf};

use systemprompt_engine::config::permission::{EDIT, SHELL};
use systemprompt_engine::config::{
    Action, ConfigError, Layout, Loaded, McpServer, Paths, Settings,
};

use crate::config::support::{paths, repo, write};

type Outcome = (Settings, Vec<PathBuf>, Vec<ConfigError>);

fn load_files(
    paths: &Paths,
    cwd: &Path,
    files: &[(&PathBuf, &str)],
) -> Result<Outcome, ConfigError> {
    for (path, text) in files {
        write(path, text);
    }
    let loaded = Loaded::load(paths, &Layout::default(), cwd, Vec::new())?;
    Ok((loaded.settings, loaded.settings_files, loaded.problems))
}

#[test]
fn global_and_project_merge() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    write(&repo.join("docs/a.md"), "a");
    write(&repo.join("docs/b.md"), "b");
    let global = paths.config.join("harness.json");
    let project = repo.join(".harness").join("harness.json");
    let (settings, read, problems) = load_files(&paths, &repo, &[
        (
            &global,
            r#"{
              // global defaults
              "model": "m-global",
              "small_model": "m-small",
              "permission": {"shell": {"*": "ask", "git *": "allow"}},
              "mcp": {
                "docs": {"type": "remote", "url": "https://docs.example/mcp", "headers": {"x-key": "k"}},
                "local": {"command": "srv", "args": ["--stdio"]},
              },
              "agent": {"build": {"temperature": 0.1, "permission": {"edit": "ask"}}},
              "instructions": ["docs/a.md"],
              "theme": "dark",
            }"#,
        ),
        (
            &project,
            r#"{
              "model": "m-project",
              "permission": {"shell": {"git push *": "deny"}},
              "mcp": {"docs": {"enabled": false}, "broken": {"type": "remote"}},
              "agent": {"build": {"steps": 9, "permission": {"edit": "allow"}}},
              "instructions": ["docs/b.md", "docs/a.md"],
              "compaction": {"auto": false},
              "experimental": {"fast": true}
            }"#,
        ),
    ])
    .unwrap();
    assert_eq!(read, [global, project.clone()]);
    assert_eq!(settings.model.as_deref(), Some("m-project"));
    assert_eq!(settings.small_model.as_deref(), Some("m-small"));
    assert_eq!(
        settings.permission.evaluate(SHELL, "git status"),
        Action::Allow
    );
    assert_eq!(
        settings.permission.evaluate(SHELL, "git push origin"),
        Action::Deny
    );
    assert_eq!(settings.permission.evaluate(SHELL, "rm -rf x"), Action::Ask);
    assert!(!settings.mcp["docs"].enabled());
    assert!(matches!(settings.mcp["local"], McpServer::Local { .. }));
    assert!(!settings.mcp.contains_key("broken"));
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].path(), Some(project.as_path()));
    let build = &settings.agents["build"];
    assert_eq!(build.temperature, Some(0.1));
    assert_eq!(build.steps, Some(9));
    assert_eq!(build.permission.evaluate(EDIT, "x"), Action::Allow);
    assert_eq!(settings.instructions, ["docs/a.md", "docs/b.md"]);
    assert!(!settings.compaction.auto);
    assert!(settings.compaction.prune);
    assert!(settings.experimental_flag("fast"));
    assert_eq!(settings.extra["theme"], "dark");
    assert_eq!(settings.enabled_mcp().count(), 1);
    assert!(!format!("{:?}", settings.mcp["docs"]).contains("\"k\""));
}

#[test]
fn invalid_files_stop_the_load_with_a_line() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    let file = repo.join("harness.json");
    let error = load_files(
        &paths,
        &repo,
        &[(&file, "{\n  \"model\": \"a\",\n  \"shell\": 4\n}")],
    )
    .unwrap_err();
    assert!(matches!(error, ConfigError::Jsonc { .. }), "{error:?}");
    assert_eq!(error.path(), Some(file.as_path()));

    let error = load_files(
        &paths,
        &repo,
        &[(&file, r#"{"permission": {"edit": "maybe"}}"#)],
    )
    .unwrap_err();
    assert!(matches!(error, ConfigError::Field { ref key, .. } if key == "permission"));

    let error = load_files(&paths, &repo, &[(&file, "[1, 2]")]).unwrap_err();
    assert!(matches!(error, ConfigError::NotAnObject { .. }));
}

#[test]
fn missing_and_empty_files() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    let empty = repo.join("harness.json");
    let (settings, read, problems) =
        load_files(&paths, &repo, &[(&empty, "  // nothing yet\n")]).unwrap();
    assert_eq!(read, [empty]);
    assert_eq!(settings, Settings::default());
    assert!(problems.is_empty());

    let other = tempfile::tempdir().unwrap();
    let bare = other.path().join("bare");
    std::fs::create_dir_all(&bare).unwrap();
    let (settings, read, _) = load_files(&paths, &bare, &[]).unwrap();
    assert_eq!(read, Vec::<PathBuf>::new());
    assert_eq!(settings, Settings::default());
}
