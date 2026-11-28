//! Settings discovery and layering, tolerant decoding of individual entries,
//! and what a merged settings set preserves.

use systemprompt_engine::config::{ConfigError, Layout, Loaded, McpServer};

use crate::support::Tree;

#[test]
fn a_settings_file_reached_both_as_global_and_by_the_project_walk_is_read_once() {
    let tree = Tree::new();
    let mut layout = Layout::default();
    layout.project_dir = ".cfg".to_owned();
    let paths = systemprompt_engine::config::Paths {
        config: tree.repo.join(".cfg"),
        ..tree.paths.clone()
    };
    tree.project(
        ".cfg/harness.json",
        r#"{"permission": {"shell": {"make *": "allow"}}}"#,
    );
    let loaded = Loaded::load(&paths, &layout, &tree.repo, Vec::new()).unwrap();
    let reads = loaded
        .settings_files
        .iter()
        .filter(|file| file.ends_with(".cfg/harness.json"))
        .count();
    assert_eq!(reads, 1, "{:?}", loaded.settings_files);
    assert_eq!(loaded.settings.permission.len(), 1);
}

#[test]
fn settings_load_from_global_then_ancestors_then_the_working_directory() {
    let tree = Tree::new();
    tree.global(
        "harness.json",
        r#"{"model": "global", "shell": "sh", "small_model": "small"}"#,
    );
    tree.project("harness.json", r#"{"model": "root", "shell": "bash"}"#);
    tree.project("pkg/harness.json", r#"{"model": "pkg"}"#);
    let loaded = tree.load_at(&tree.repo.join("pkg"));
    assert_eq!(loaded.settings.model.as_deref(), Some("pkg"));
    assert_eq!(loaded.settings.shell.as_deref(), Some("bash"));
    assert_eq!(loaded.settings.small_model.as_deref(), Some("small"));
    let order: Vec<String> = loaded
        .settings_files
        .iter()
        .map(|file| {
            file.strip_prefix(tree.repo.parent().unwrap())
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    assert_eq!(
        order,
        [
            "config/harness.json",
            "repo/harness.json",
            "repo/pkg/harness.json"
        ]
    );
}

#[test]
fn comments_and_trailing_commas_are_allowed_in_settings() {
    let tree = Tree::new();
    tree.project(
        "harness.json",
        "{\n  // the model\n  \"model\": \"m\", /* trailing */\n  \"shell\": \"sh\",\n}\n",
    );
    let loaded = tree.load();
    assert_eq!(loaded.settings.model.as_deref(), Some("m"));
}

#[test]
fn a_legacy_config_file_name_is_not_loaded() {
    let tree = Tree::new();
    tree.global("config.json", r#"{"model": "legacy"}"#);
    tree.project("config.json", r#"{"model": "legacy"}"#);
    assert_eq!(tree.load().settings.model, None);
}

#[test]
fn a_schema_key_is_accepted_and_kept_as_written() {
    let tree = Tree::new();
    tree.project(
        "harness.json",
        r#"{"$schema": "https://example.test/schema.json", "model": "m"}"#,
    );
    let loaded = tree.load();
    assert_eq!(loaded.settings.model.as_deref(), Some("m"));
    assert_eq!(
        loaded.settings.extra["$schema"],
        "https://example.test/schema.json"
    );
}

#[test]
fn a_non_object_root_is_refused_with_a_single_error_naming_the_file() {
    for body in ["[]", "3", "\"text\"", "true"] {
        let tree = Tree::new();
        let file = tree.project("harness.json", body);
        let error =
            Loaded::load(&tree.paths, &Layout::default(), &tree.repo, Vec::new()).unwrap_err();
        assert!(
            matches!(&error, ConfigError::NotAnObject { path } if *path == file),
            "{body}: {error}"
        );
    }
}

#[test]
fn a_malformed_server_entry_is_dropped_alone_and_reported() {
    let tree = Tree::new();
    tree.project(
        "harness.json",
        r#"{"mcp": {"good": {"type": "local", "command": "srv"}, "bad": {"type": "remote"}, "odd": {"type": "carrier-pigeon"}}}"#,
    );
    let loaded = tree.load();
    assert_eq!(loaded.settings.mcp.keys().collect::<Vec<_>>(), ["good"]);
    assert_eq!(loaded.problems.len(), 2, "{:?}", loaded.problems);
}

#[test]
fn formatter_and_language_server_entries_without_a_command_are_dropped_while_an_empty_map_stays_empty()
 {
    let tree = Tree::new();
    tree.project(
        "harness.json",
        r#"{"formatter": {"none": {"command": ""}, "off": {"disabled": true}}, "lsp": {}}"#,
    );
    let loaded = tree.load();
    assert!(loaded.settings.formatters.is_empty());
    assert!(loaded.settings.lsp.is_empty());
    assert_eq!(loaded.problems.len(), 1, "{:?}", loaded.problems);
}

#[test]
fn a_numeric_server_timeout_is_kept_for_every_phase_of_that_server() {
    let tree = Tree::new();
    tree.project(
        "harness.json",
        r#"{"mcp": {"srv": {"type": "remote", "url": "https://mcp.example.test", "timeout_ms": 2500}}}"#,
    );
    let loaded = tree.load();
    assert_eq!(loaded.settings.mcp["srv"].timeout_ms(), Some(2500));
}

#[test]
fn explicit_false_zero_and_empty_values_are_preserved() {
    let tree = Tree::new();
    tree.global(
        "harness.json",
        r#"{"compaction": {"auto": true, "reserved_tokens": 9000}, "instructions": ["a.md"]}"#,
    );
    tree.project(
        "harness.json",
        r#"{"compaction": {"auto": false, "prune": false, "reserved_tokens": 0}, "mcp": {"srv": {"command": "s", "enabled": false}}, "experimental": {"x": false}}"#,
    );
    let loaded = tree.load();
    let compaction = loaded.settings.compaction;
    assert!(!compaction.auto);
    assert!(!compaction.prune);
    assert_eq!(compaction.reserved_tokens, Some(0));
    assert!(!loaded.settings.mcp["srv"].enabled());
    assert_eq!(loaded.settings.enabled_mcp().count(), 0);
    assert_eq!(loaded.settings.experimental["x"], false);
}

#[test]
fn compaction_settings_merge_leaf_by_leaf_across_layers() {
    let tree = Tree::new();
    tree.global(
        "harness.json",
        r#"{"compaction": {"auto": false, "reserved_tokens": 4000}}"#,
    );
    tree.project("harness.json", r#"{"compaction": {"prune": false}}"#);
    let compaction = tree.load().settings.compaction;
    assert!(!compaction.auto);
    assert!(!compaction.prune);
    assert_eq!(compaction.reserved_tokens, Some(4000));
}

#[test]
fn server_credentials_never_appear_in_debug_output() {
    let tree = Tree::new();
    tree.project(
        "harness.json",
        r#"{"mcp": {"srv": {"url": "https://mcp.example.test", "headers": {"Authorization": "Bearer sekrit-value"}}, "loc": {"command": "x", "env": {"TOKEN": "env-sekrit"}}}}"#,
    );
    let loaded = tree.load();
    let shown = format!("{:?}", loaded.settings);
    assert!(!shown.contains("sekrit"), "{shown}");
    assert!(shown.contains("Authorization"));
    assert!(matches!(
        loaded.settings.mcp["srv"],
        McpServer::Remote { .. }
    ));
}

#[test]
fn configured_language_servers_merge_by_name_across_layers() {
    let tree = Tree::new();
    tree.global(
        "harness.json",
        r#"{"lsp": {"rust": {"command": "ra", "extensions": [".rs"]}, "go": {"command": "gopls"}}}"#,
    );
    tree.project(
        "harness.json",
        r#"{"lsp": {"rust": {"args": ["--log"]}, "py": {"command": "pyls"}}}"#,
    );
    let lsp = tree.load().settings.lsp;
    assert_eq!(lsp.keys().collect::<Vec<_>>(), ["rust", "go", "py"]);
    assert_eq!(lsp["rust"].command, "ra");
    assert_eq!(lsp["rust"].args, ["--log"]);
}

#[test]
fn no_language_server_is_invented_when_the_setting_is_absent() {
    let tree = Tree::new();
    tree.project("harness.json", r#"{"model": "m"}"#);
    assert!(tree.load().settings.lsp.is_empty());
}

#[test]
fn a_disabled_language_server_in_a_nearer_layer_does_not_revive_an_earlier_entry() {
    let tree = Tree::new();
    tree.global("harness.json", r#"{"lsp": {"rust": {"command": "ra"}}}"#);
    tree.project("harness.json", r#"{"lsp": {"rust": {"disabled": true}}}"#);
    let lsp = tree.load().settings.lsp;
    assert!(
        lsp.get("rust").is_none_or(|server| server.disabled),
        "{lsp:?}"
    );
}

#[test]
fn a_model_selection_is_kept_as_written_in_short_and_explicit_forms() {
    let tree = Tree::new();
    tree.project(
        "harness.json",
        r#"{"model": "gateway/claude-sonnet", "small_model": "haiku"}"#,
    );
    let settings = tree.load().settings;
    assert_eq!(settings.model.as_deref(), Some("gateway/claude-sonnet"));
    assert_eq!(settings.small_model.as_deref(), Some("haiku"));
}

#[test]
fn a_model_selection_that_is_not_text_fails_the_load() {
    let tree = Tree::new();
    tree.project("harness.json", r#"{"model": {"id": 3}}"#);
    assert!(Loaded::load(&tree.paths, &Layout::default(), &tree.repo, Vec::new()).is_err());
}

#[test]
#[ignore = "parity ledger: an invalid settings file fails the whole load instead of being skipped"]
fn an_invalid_file_is_skipped_while_valid_files_still_load() {
    let tree = Tree::new();
    tree.global("harness.json", r#"{"model": "global"}"#);
    tree.project("harness.json", "{ not json");
    let loaded = Loaded::load(&tree.paths, &Layout::default(), &tree.repo, Vec::new());
    assert_eq!(loaded.unwrap().settings.model.as_deref(), Some("global"));
}

#[test]
#[ignore = "parity ledger: settings values are not substituted from files or the environment"]
fn file_substitutions_in_settings_values_are_resolved() {
    let tree = Tree::new();
    tree.project("model.txt", "from-file");
    tree.project("harness.json", r#"{"model": "{file:./model.txt}"}"#);
    assert_eq!(tree.load().settings.model.as_deref(), Some("from-file"));
}

#[test]
#[ignore = "parity ledger: settings above the project root are not read"]
fn settings_above_the_project_root_still_apply() {
    let tree = Tree::new();
    tree.put(
        &tree.repo.parent().unwrap().join("harness.json"),
        r#"{"model": "ancestor"}"#,
    );
    assert_eq!(tree.load().settings.model.as_deref(), Some("ancestor"));
}

#[test]
fn a_settings_file_nested_beyond_reason_is_reported_instead_of_crashing() {
    let tree = Tree::new();
    let depth = 4000;
    let body = format!("{{\"extra\": {}{}}}", "[".repeat(depth), "]".repeat(depth));
    tree.project("harness.json", &body);
    let outcome = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn({
            let paths = tree.paths.clone();
            let repo = tree.repo.clone();
            move || Loaded::load(&paths, &Layout::default(), &repo, Vec::new()).map(|_| ())
        })
        .unwrap()
        .join();
    assert!(outcome.is_ok(), "loading a deeply nested file panicked");
}

#[test]
fn turning_compatibility_directories_off_leaves_only_the_harness_locations() {
    let tree = Tree::new();
    tree.put(&tree.paths.home.join(".claude/CLAUDE.md"), "home compat");
    tree.project(".claude/commands/c.md", "compat command");
    tree.project(".harness/commands/h.md", "own command");
    let layout = Layout {
        compat_dirs: false,
        ..Layout::default()
    };
    let loaded = tree.load_with(&layout);
    assert_eq!(loaded.commands.keys().collect::<Vec<_>>(), ["h"]);
    assert!(loaded.instructions.is_empty());
}
