//! Settings that steer the tools: the preferred shell.

use std::path::Path;

use systemprompt_engine::config::{Layout, Loaded, Paths};
use systemprompt_engine::tools::ShellSpec;

fn paths(root: &Path) -> Paths {
    let dir = |name: &str| root.join(name);
    Paths {
        home: dir("home"),
        data: dir("data"),
        config: dir("config"),
        cache: dir("cache"),
        state: dir("state"),
        tmp: dir("tmp"),
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn the_preferred_shell_is_read_from_settings_and_a_change_is_seen_on_reload() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let first = tmp.path().join("shells").join("first-sh");
    let second = tmp.path().join("shells").join("pwsh");
    write(&first, "");
    write(&second, "");
    let settings =
        |shell: &Path| serde_json::json!({ "shell": shell.to_string_lossy() }).to_string();
    write(&repo.join("harness.json"), &settings(&first));
    let loaded = Loaded::load(&paths, &Layout::default(), &repo, Vec::new()).unwrap();
    let preferred = loaded.settings.shell.clone().unwrap();
    let spec = ShellSpec::detect(Some(Path::new(&preferred)));
    assert_eq!(spec.path, first);

    std::thread::sleep(std::time::Duration::from_millis(20));
    write(&repo.join("harness.json"), &settings(&second));
    assert!(loaded.is_stale());
    let reloaded = Loaded::load(&paths, &Layout::default(), &repo, Vec::new()).unwrap();
    let spec = ShellSpec::detect(reloaded.settings.shell.as_deref().map(Path::new));
    assert_eq!(spec.path, second);
    assert_eq!(spec.kind, systemprompt_engine::tools::ShellKind::Pwsh);
}
