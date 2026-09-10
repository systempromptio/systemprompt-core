use std::path::{Path, PathBuf};

use systemprompt_marketplace::{
    ImportOptions, ImportReport, ImportWarning, MarketplaceError, import_anthropic_tree,
};
use tempfile::TempDir;

pub(crate) struct Tree {
    dir: TempDir,
}

impl Tree {
    fn new() -> Self {
        Self {
            dir: TempDir::new().expect("tempdir"),
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, rel: &str, body: &str) -> &Self {
        let path = self.dir.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("relative path has a parent"))
            .expect("create parent");
        std::fs::write(&path, body).expect("write file");
        self
    }

    fn mkdir(&self, rel: &str) -> &Self {
        std::fs::create_dir_all(self.dir.path().join(rel)).expect("mkdir");
        self
    }
}

fn marketplace_json(plugins: &str) -> String {
    format!(r#"{{"name":"acme","owner":{{"name":"Acme"}},"plugins":[{plugins}]}}"#)
}

fn skill_md(description: &str) -> String {
    format!("---\nname: a-skill\ndescription: {description}\n---\n\nBody.\n")
}

fn run(tree: &Tree) -> Result<(TempDir, ImportReport), MarketplaceError> {
    let dest = TempDir::new().expect("tempdir");
    let report = import_anthropic_tree(tree.path(), dest.path(), &ImportOptions::default())?;
    Ok((dest, report))
}

fn expect_ok(tree: &Tree) -> (TempDir, ImportReport) {
    run(tree).expect("import succeeds")
}

fn expect_err(tree: &Tree) -> String {
    run(tree).expect_err("import must fail").to_string()
}

fn plugin_yaml(dest: &Path, id: &str) -> serde_yaml::Value {
    let text = std::fs::read_to_string(dest.join("plugins").join(id).join("config.yaml"))
        .expect("plugin config written");
    serde_yaml::from_str(&text).expect("valid yaml")
}

fn minimal_plugin(tree: &Tree, dir: &str, manifest: &str) {
    tree.write(&format!("{dir}/.claude-plugin/plugin.json"), manifest);
    tree.write(
        &format!("{dir}/skills/a-skill/SKILL.md"),
        &skill_md("What it does."),
    );
}

#[test]
fn a_source_tree_that_does_not_exist_is_refused() {
    let dest = TempDir::new().expect("tempdir");
    let err = import_anthropic_tree(
        &PathBuf::from("/nonexistent/anthropic-tree"),
        dest.path(),
        &ImportOptions::default(),
    )
    .expect_err("missing source must fail");

    assert!(err.to_string().contains("does not exist"), "{err}");
}

#[test]
fn a_destination_that_is_a_file_is_refused() {
    let tree = Tree::new();
    let holder = TempDir::new().expect("tempdir");
    let dest = holder.path().join("out");
    std::fs::write(&dest, b"x").expect("write");

    let err = import_anthropic_tree(tree.path(), &dest, &ImportOptions::default())
        .expect_err("a file destination must fail");

    assert!(err.to_string().contains("not a directory"), "{err}");
}

#[test]
fn a_remote_plugin_source_is_skipped_and_reported() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","source":{"type":"git","repo":"a/b"}}"#),
    );

    let (_dest, report) = expect_ok(&tree);

    assert!(report.plugins.is_empty());
    assert!(
        report
            .warnings
            .contains(&ImportWarning::RemotePluginSource {
                plugin: "alpha".to_owned()
            })
    );
}

#[test]
fn plugin_root_metadata_relocates_where_plugins_are_found() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        r#"{"name":"acme","metadata":{"pluginRoot":"./pkgs"},"plugins":[{"name":"alpha"}]}"#,
    );
    minimal_plugin(&tree, "pkgs/alpha", r#"{"name":"alpha","version":"2.0.0"}"#);

    let (dest, report) = expect_ok(&tree);

    assert_eq!(
        report
            .plugins
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha"]
    );
    assert_eq!(
        plugin_yaml(dest.path(), "alpha")["plugin"]["version"].as_str(),
        Some("2.0.0")
    );
}

#[test]
fn an_entry_source_path_overrides_the_plugin_root() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        r#"{"name":"acme","metadata":{"pluginRoot":"./pkgs"},"plugins":[{"name":"alpha","source":"./elsewhere/alpha"}]}"#,
    );
    minimal_plugin(&tree, "elsewhere/alpha", r#"{"name":"alpha"}"#);

    let (_dest, report) = expect_ok(&tree);

    assert_eq!(report.plugins.len(), 1);
}

#[test]
fn a_plugin_without_a_manifest_names_the_missing_file() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha"}"#),
    );
    tree.mkdir("plugins/alpha");

    let message = expect_err(&tree);

    assert!(message.contains("plugin.json"), "{message}");
}

#[test]
fn an_unparsable_plugin_manifest_says_so() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha"}"#),
    );
    tree.write("plugins/alpha/.claude-plugin/plugin.json", "{ not json");

    let message = expect_err(&tree);

    assert!(message.contains("plugin.json is not valid"), "{message}");
}

#[test]
fn an_unparsable_marketplace_manifest_says_so() {
    let tree = Tree::new();
    tree.write(".claude-plugin/marketplace.json", "{ not json");

    let message = expect_err(&tree);

    assert!(
        message.contains("marketplace.json is not valid"),
        "{message}"
    );
}

#[test]
fn a_plugin_with_no_category_anywhere_falls_back_and_warns() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha"}"#),
    );
    minimal_plugin(&tree, "plugins/alpha", r#"{"name":"alpha"}"#);

    let (dest, report) = expect_ok(&tree);

    assert!(report.warnings.contains(&ImportWarning::MissingCategory {
        plugin: "alpha".to_owned(),
        applied: "general".to_owned(),
    }));
    assert_eq!(
        plugin_yaml(dest.path(), "alpha")["plugin"]["category"].as_str(),
        Some("general")
    );
}

#[test]
fn a_blank_marketplace_category_is_treated_as_absent() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"   "}"#),
    );
    minimal_plugin(&tree, "plugins/alpha", r#"{"name":"alpha"}"#);

    let (_dest, report) = expect_ok(&tree);

    assert!(report.warnings.iter().any(|w| matches!(
        w,
        ImportWarning::MissingCategory { applied, .. } if applied == "general"
    )));
}

#[test]
fn a_plugin_shipping_no_skills_is_reported() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"ops"}"#),
    );
    tree.write(
        "plugins/alpha/.claude-plugin/plugin.json",
        r#"{"name":"alpha"}"#,
    );

    let (_dest, report) = expect_ok(&tree);

    assert!(report.warnings.contains(&ImportWarning::NoSkills {
        plugin: "alpha".to_owned()
    }));
    assert!(report.skills.is_empty());
}

#[test]
fn inline_mcp_json_and_a_commands_directory_are_both_reported() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"ops"}"#),
    );
    minimal_plugin(&tree, "plugins/alpha", r#"{"name":"alpha"}"#);
    tree.write("plugins/alpha/.mcp.json", r#"{"mcpServers":{}}"#);
    tree.write("plugins/alpha/commands/deploy.md", "run it\n");

    let (_dest, report) = expect_ok(&tree);

    assert!(report.warnings.contains(&ImportWarning::InlineMcpServers {
        plugin: "alpha".to_owned()
    }));
    assert!(report.warnings.contains(&ImportWarning::CommandsDirectory {
        plugin: "alpha".to_owned()
    }));
}

#[test]
fn agent_markdown_files_are_counted_not_imported() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"ops"}"#),
    );
    minimal_plugin(&tree, "plugins/alpha", r#"{"name":"alpha"}"#);
    tree.write("plugins/alpha/agents/one.md", "a\n");
    tree.write("plugins/alpha/agents/two.md", "b\n");
    tree.write("plugins/alpha/agents/notes.txt", "ignored\n");

    let (dest, report) = expect_ok(&tree);

    assert!(report.warnings.contains(&ImportWarning::AgentsDirectory {
        plugin: "alpha".to_owned(),
        count: 2,
    }));
    assert!(!dest.path().join("agents").exists());
}

#[test]
fn plugin_facts_fall_back_to_the_marketplace_entry_then_to_defaults() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(
            r#"{"name":"alpha","category":"ops","description":"From the entry.","version":"3.1.0","license":"MIT","author":{"name":"Acme","email":"a@b.c"},"keywords":["b"],"tags":["a","b"]}"#,
        ),
    );
    minimal_plugin(
        &tree,
        "plugins/alpha",
        r#"{"name":"alpha","description":"","version":""}"#,
    );

    let (dest, _report) = expect_ok(&tree);
    let doc = plugin_yaml(dest.path(), "alpha");
    let plugin = &doc["plugin"];

    assert_eq!(plugin["description"].as_str(), Some("From the entry."));
    assert_eq!(plugin["version"].as_str(), Some("3.1.0"));
    assert_eq!(plugin["license"].as_str(), Some("MIT"));
    assert_eq!(plugin["author"]["name"].as_str(), Some("Acme"));
    assert_eq!(plugin["author"]["email"].as_str(), Some("a@b.c"));
    assert_eq!(
        plugin["keywords"]
            .as_sequence()
            .expect("keywords")
            .iter()
            .filter_map(serde_yaml::Value::as_str)
            .collect::<Vec<_>>(),
        vec!["a", "b"],
        "entry keywords and tags merge, sorted and deduped"
    );
}

#[test]
fn an_entry_with_no_version_or_license_gets_the_importer_defaults() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"ops","version":"  "}"#),
    );
    minimal_plugin(&tree, "plugins/alpha", r#"{"name":"alpha"}"#);

    let (dest, _report) = expect_ok(&tree);
    let doc = plugin_yaml(dest.path(), "alpha");

    assert_eq!(doc["plugin"]["version"].as_str(), Some("0.1.0"));
    assert_eq!(doc["plugin"]["license"].as_str(), Some("proprietary"));
}

#[test]
fn a_manifest_that_states_its_own_facts_wins_over_the_entry() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(
            r#"{"name":"alpha","category":"ops","description":"entry","version":"3.1.0","keywords":["entry"]}"#,
        ),
    );
    minimal_plugin(
        &tree,
        "plugins/alpha",
        r#"{"name":"alpha","description":"manifest","version":"9.9.9","keywords":["manifest"],"license":"Apache-2.0"}"#,
    );

    let (dest, _report) = expect_ok(&tree);
    let plugin = plugin_yaml(dest.path(), "alpha")["plugin"].clone();

    assert_eq!(plugin["description"].as_str(), Some("manifest"));
    assert_eq!(plugin["version"].as_str(), Some("9.9.9"));
    assert_eq!(plugin["license"].as_str(), Some("Apache-2.0"));
    assert_eq!(plugin["keywords"].as_sequence().expect("keywords").len(), 1);
}

#[test]
fn a_skill_id_claimed_by_two_plugins_is_refused() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"ops"},{"name":"beta","category":"ops"}"#),
    );
    minimal_plugin(&tree, "plugins/alpha", r#"{"name":"alpha"}"#);
    minimal_plugin(&tree, "plugins/beta", r#"{"name":"beta"}"#);

    let message = expect_err(&tree);

    assert!(message.contains("more than one plugin"), "{message}");
    assert!(message.contains("a_skill"), "{message}");
}

#[test]
fn a_rule_name_claimed_twice_across_spellings_is_refused() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"ops"}"#),
    );
    minimal_plugin(&tree, "plugins/alpha", r#"{"name":"alpha"}"#);
    tree.write("plugins/alpha/rules/hand-over.md", "one\n");
    tree.write("plugins/alpha/rules/hand_over.md", "two\n");

    let message = expect_err(&tree);

    assert!(message.contains("hand_over"), "{message}");
    assert!(message.contains("more than once"), "{message}");
}

#[test]
fn rule_frontmatter_supplies_the_display_name_and_description() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"ops"}"#),
    );
    minimal_plugin(&tree, "plugins/alpha", r#"{"name":"alpha"}"#);
    tree.write(
        "plugins/alpha/rules/hand-over.md",
        "---\nname: Handover Protocol\ndescription: How to hand over.\n---\n\nBody.\n",
    );

    let (dest, report) = expect_ok(&tree);
    let text = std::fs::read_to_string(dest.path().join("rules/hand_over/config.yaml"))
        .expect("rule config");

    assert_eq!(report.rules, vec!["hand_over"]);
    assert!(text.contains("name: Handover Protocol"), "{text}");
    assert!(text.contains("description: How to hand over."), "{text}");
}

#[test]
fn a_rule_without_frontmatter_gets_a_name_derived_from_its_filename() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"ops"}"#),
    );
    minimal_plugin(&tree, "plugins/alpha", r#"{"name":"alpha"}"#);
    tree.write("plugins/alpha/rules/hand-over.md", "Just a body.\n");

    let (dest, _report) = expect_ok(&tree);
    let text = std::fs::read_to_string(dest.path().join("rules/hand_over/config.yaml"))
        .expect("rule config");

    assert!(text.contains("name: hand over"), "{text}");
    assert!(text.contains("description: ''"), "{text}");
}

#[test]
fn a_blank_rule_frontmatter_name_falls_back_to_the_filename() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"ops"}"#),
    );
    minimal_plugin(&tree, "plugins/alpha", r#"{"name":"alpha"}"#);
    tree.write(
        "plugins/alpha/rules/hand-over.md",
        "---\nname: '   '\n---\n\nBody.\n",
    );

    let (dest, _report) = expect_ok(&tree);
    let text = std::fs::read_to_string(dest.path().join("rules/hand_over/config.yaml"))
        .expect("rule config");

    assert!(text.contains("name: hand over"), "{text}");
}

#[test]
fn unparsable_rule_frontmatter_names_the_file() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"ops"}"#),
    );
    minimal_plugin(&tree, "plugins/alpha", r#"{"name":"alpha"}"#);
    tree.write(
        "plugins/alpha/rules/broken.md",
        "---\nname: [unclosed\n---\n\nBody.\n",
    );

    let message = expect_err(&tree);

    assert!(
        message.contains("rule frontmatter is not valid YAML"),
        "{message}"
    );
    assert!(message.contains("broken.md"), "{message}");
}

#[test]
fn non_markdown_files_in_a_rules_directory_are_ignored() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"ops"}"#),
    );
    minimal_plugin(&tree, "plugins/alpha", r#"{"name":"alpha"}"#);
    tree.write("plugins/alpha/rules/README.txt", "not a rule\n");
    tree.mkdir("plugins/alpha/rules/subdir");

    let (_dest, report) = expect_ok(&tree);

    assert!(report.rules.is_empty());
}

fn tree_with_skill(body: &str) -> Tree {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        &marketplace_json(r#"{"name":"alpha","category":"ops"}"#),
    );
    tree.write(
        "plugins/alpha/.claude-plugin/plugin.json",
        r#"{"name":"alpha"}"#,
    );
    tree.write("plugins/alpha/skills/a-skill/SKILL.md", body);
    tree
}

fn skill_yaml(dest: &Path) -> serde_yaml::Value {
    let text = std::fs::read_to_string(dest.join("skills/a_skill/config.yaml"))
        .expect("skill config written");
    serde_yaml::from_str(&text).expect("valid yaml")
}

#[test]
fn a_skill_without_frontmatter_is_refused_for_having_no_description() {
    let tree = tree_with_skill("Just a body, no frontmatter.\n");

    let message = expect_err(&tree);

    assert!(message.contains("non-empty 'description'"), "{message}");
}

#[test]
fn a_blank_skill_description_is_refused() {
    let tree = tree_with_skill("---\nname: a-skill\ndescription: '   '\n---\n\nBody.\n");

    let message = expect_err(&tree);

    assert!(message.contains("non-empty 'description'"), "{message}");
}

#[test]
fn unparsable_skill_frontmatter_names_the_file() {
    let tree = tree_with_skill("---\ndescription: [unclosed\n---\n\nBody.\n");

    let message = expect_err(&tree);

    assert!(
        message.contains("SKILL.md frontmatter is not valid YAML"),
        "{message}"
    );
}

#[test]
fn a_skill_with_only_a_description_takes_its_display_name_from_the_directory() {
    let tree = tree_with_skill("---\ndescription: What it does.\n---\n\nBody.\n");

    let (dest, _report) = expect_ok(&tree);

    assert_eq!(skill_yaml(dest.path())["name"].as_str(), Some("a skill"));
}

#[test]
fn the_frontmatter_name_is_the_display_name_when_no_title_is_given() {
    let tree = tree_with_skill("---\nname: a-skill\ndescription: d\n---\n\nBody.\n");

    let (dest, _report) = expect_ok(&tree);

    assert_eq!(skill_yaml(dest.path())["name"].as_str(), Some("a-skill"));
}

#[test]
fn a_title_beats_the_frontmatter_name() {
    let tree = tree_with_skill("---\nname: a-skill\ntitle: A Skill\ndescription: d\n---\nB.\n");

    let (dest, _report) = expect_ok(&tree);

    assert_eq!(skill_yaml(dest.path())["name"].as_str(), Some("A Skill"));
}

#[test]
fn a_comma_separated_tag_string_becomes_a_tag_list() {
    let tree = tree_with_skill("---\ndescription: d\ntags: 'one, two ,, three'\n---\nB.\n");

    let (dest, _report) = expect_ok(&tree);
    let tags: Vec<String> =
        serde_yaml::from_value(skill_yaml(dest.path())["tags"].clone()).expect("tags is a list");

    assert_eq!(tags, vec!["one", "two", "three"]);
}

#[test]
fn a_tag_sequence_is_taken_as_written() {
    let tree = tree_with_skill("---\ndescription: d\ntags: [one, two]\n---\nB.\n");

    let (dest, _report) = expect_ok(&tree);
    let tags: Vec<String> =
        serde_yaml::from_value(skill_yaml(dest.path())["tags"].clone()).expect("tags is a list");

    assert_eq!(tags, vec!["one", "two"]);
}

#[test]
fn a_skill_category_overrides_the_plugin_category_and_hosts_survive() {
    let tree = tree_with_skill(
        "---\ndescription: d\ncategory: research\ndisplay_category: Research\nhosts: \
         [claude-code]\n---\nB.\n",
    );

    let (dest, _report) = expect_ok(&tree);
    let doc = skill_yaml(dest.path());

    assert_eq!(doc["category"].as_str(), Some("research"));
    assert_eq!(doc["display_category"].as_str(), Some("Research"));
    assert_eq!(doc["hosts"][0].as_str(), Some("claude-code"));
}

#[test]
fn a_skills_directory_entry_without_a_skill_file_is_not_a_skill() {
    let tree = tree_with_skill("---\ndescription: d\n---\nB.\n");
    tree.write("plugins/alpha/skills/not-a-skill/README.md", "nope\n");

    let (_dest, report) = expect_ok(&tree);

    assert_eq!(report.skills, vec!["a_skill"]);
}

fn tree_with_hooks(hooks_json: &str) -> Tree {
    let tree = tree_with_skill("---\ndescription: d\n---\nB.\n");
    tree.write("plugins/alpha/hooks/hooks.json", hooks_json);
    tree
}

#[test]
fn a_non_command_hook_action_is_reported_and_dropped() {
    let tree = tree_with_hooks(
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"prompt","prompt":"ask"}]}]}}"#,
    );

    let (_dest, report) = expect_ok(&tree);

    assert!(report.hooks.is_empty());
    assert!(
        report
            .warnings
            .contains(&ImportWarning::UnsupportedHookAction {
                plugin: "alpha".to_owned(),
                event: "PreToolUse".to_owned(),
            })
    );
}

#[test]
fn a_command_hook_with_a_blank_command_is_reported_and_dropped() {
    let tree = tree_with_hooks(
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"  "}]}]}}"#,
    );

    let (_dest, report) = expect_ok(&tree);

    assert!(report.hooks.is_empty());
    assert!(report.warnings.iter().any(|w| matches!(
        w,
        ImportWarning::UnsupportedHookAction { event, .. } if event == "PreToolUse"
    )));
}

#[test]
fn several_actions_under_one_event_get_distinct_indexed_directories() {
    let tree = tree_with_hooks(
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"one"},{"type":"command","command":"two","async":true}]}]}}"#,
    );

    let (dest, report) = expect_ok(&tree);

    assert_eq!(
        report.hooks,
        vec!["alpha__PreToolUse__0", "alpha__PreToolUse__1"]
    );
    let second =
        std::fs::read_to_string(dest.path().join("hooks/alpha__PreToolUse__1/config.yaml"))
            .expect("second hook written");
    let doc: serde_yaml::Value = serde_yaml::from_str(&second).expect("valid yaml");
    assert_eq!(doc["command"].as_str(), Some("two"));
    assert_eq!(doc["async"].as_bool(), Some(true));
    assert_eq!(doc["name"].as_str(), Some("alpha PreToolUse 1"));
    assert_eq!(doc["tags"][0].as_str(), Some("alpha"));
}

#[test]
fn an_unparsable_hooks_file_names_it() {
    let tree = tree_with_hooks("{ not json");

    let message = expect_err(&tree);

    assert!(message.contains("hooks.json is not valid"), "{message}");
}

#[test]
fn imported_hook_ids_are_added_to_the_plugin_hook_references() {
    let tree = tree_with_hooks(
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"one"}]}]}}"#,
    );

    let (dest, _report) = expect_ok(&tree);
    let include: Vec<String> = serde_yaml::from_value(
        plugin_yaml(dest.path(), "alpha")["plugin"]["hooks"]["include"].clone(),
    )
    .expect("hook include list");

    assert_eq!(include, vec!["alpha__PreToolUse__0"]);
}

#[test]
fn a_base_directory_that_is_not_a_bundle_directory_is_refused() {
    let tree = Tree::new();
    tree.write("systemprompt/nonsense/a.yaml", "x: 1\n");

    let message = expect_err(&tree);

    assert!(
        message.contains("not a services bundle directory"),
        "{message}"
    );
    assert!(message.contains("nonsense"), "{message}");
}

#[test]
fn a_base_tree_may_not_restate_what_is_authored_in_anthropic_form() {
    let tree = Tree::new();
    tree.write("systemprompt/skills/a_skill/config.yaml", "id: a_skill\n");

    let message = expect_err(&tree);

    assert!(message.contains("authored in Anthropic form"), "{message}");
    assert!(message.contains("skills"), "{message}");
}

#[test]
fn loose_files_beside_the_base_directories_are_ignored() {
    let tree = Tree::new();
    tree.write("systemprompt/README.md", "not a directory\n");
    tree.write("systemprompt/mcp/one.yaml", "mcp_servers: {}\n");

    let (dest, report) = expect_ok(&tree);

    assert_eq!(report.copied_base_dirs, vec!["mcp"]);
    assert!(!dest.path().join("README.md").exists());
}

#[test]
fn base_directories_are_copied_in_sorted_order_with_their_subtrees() {
    let tree = Tree::new();
    tree.write("systemprompt/mcp/one.yaml", "mcp_servers: {}\n");
    tree.write("systemprompt/agents/nested/deep/agent.yaml", "agents: {}\n");

    let (dest, report) = expect_ok(&tree);

    assert_eq!(report.copied_base_dirs, vec!["agents", "mcp"]);
    assert!(dest.path().join("agents/nested/deep/agent.yaml").is_file());
}

#[test]
fn the_aggregator_lists_every_top_level_yaml_of_the_copied_base_dirs() {
    let tree = Tree::new();
    tree.write("systemprompt/mcp/zeta.yaml", "mcp_servers: {}\n");
    tree.write("systemprompt/mcp/alpha.yml", "mcp_servers: {}\n");
    tree.write("systemprompt/mcp/notes.md", "ignored\n");
    tree.write("systemprompt/mcp/nested/deeper.yaml", "mcp_servers: {}\n");

    let (dest, _report) = expect_ok(&tree);
    let text = std::fs::read_to_string(dest.path().join("config/config.yaml"))
        .expect("aggregator written");

    assert!(text.contains("- ../mcp/alpha.yml"), "{text}");
    assert!(text.contains("- ../mcp/zeta.yaml"), "{text}");
    assert!(!text.contains("notes.md"), "{text}");
    assert!(!text.contains("deeper.yaml"), "{text}");
    assert!(text.contains("agent_port_range"), "{text}");
}

#[test]
fn a_base_tree_that_ships_its_own_root_config_keeps_it() {
    let tree = Tree::new();
    tree.write(
        "systemprompt/config/config.yaml",
        "includes: []\nsettings: {}\n",
    );
    tree.write("systemprompt/mcp/one.yaml", "mcp_servers: {}\n");

    let (dest, _report) = expect_ok(&tree);
    let text =
        std::fs::read_to_string(dest.path().join("config/config.yaml")).expect("base config kept");

    assert!(!text.contains("../mcp/one.yaml"), "{text}");
    assert!(text.contains("settings: {}"), "{text}");
}

#[test]
fn a_declared_script_whose_file_is_missing_is_refused() {
    let tree = tree_with_skill("---\ndescription: d\n---\nB.\n");
    tree.write(
        "plugins/alpha/.claude-plugin/systemprompt.yaml",
        "schema: 1\nplugin:\n  category: ops\n  scripts:\n    - name: setup\n      source: \
         scripts/setup.sh\n",
    );

    let message = expect_err(&tree);

    assert!(message.contains("declares script 'setup'"), "{message}");
    assert!(message.contains("missing"), "{message}");
}

#[test]
fn a_declared_script_is_copied_under_the_plugin() {
    let tree = tree_with_skill("---\ndescription: d\n---\nB.\n");
    tree.write("plugins/alpha/scripts/setup.sh", "#!/bin/sh\necho hi\n");
    tree.write(
        "plugins/alpha/.claude-plugin/systemprompt.yaml",
        "schema: 1\nplugin:\n  category: ops\n  scripts:\n    - name: setup\n      source: \
         scripts/setup.sh\n",
    );

    let (dest, _report) = expect_ok(&tree);

    assert!(dest.path().join("plugins/alpha/scripts/setup.sh").is_file());
}

#[test]
fn a_sidecar_without_a_schema_key_is_refused() {
    let tree = tree_with_skill("---\ndescription: d\n---\nB.\n");
    tree.write(
        "plugins/alpha/.claude-plugin/systemprompt.yaml",
        "plugin:\n  category: ops\n",
    );

    let message = expect_err(&tree);

    assert!(message.contains("missing 'schema: 1'"), "{message}");
}

#[test]
fn a_sidecar_from_a_future_schema_is_refused_by_version() {
    let tree = tree_with_skill("---\ndescription: d\n---\nB.\n");
    tree.write(
        "plugins/alpha/.claude-plugin/systemprompt.yaml",
        "schema: 7\nplugin:\n  category: ops\n",
    );

    let message = expect_err(&tree);

    assert!(message.contains("schema 7 is not supported"), "{message}");
}

#[test]
fn an_unknown_sidecar_key_is_refused() {
    let tree = tree_with_skill("---\ndescription: d\n---\nB.\n");
    tree.write(
        "plugins/alpha/.claude-plugin/systemprompt.yaml",
        "schema: 1\nplugin:\n  category: ops\n  nonsense: true\n",
    );

    let message = expect_err(&tree);

    assert!(message.contains("nonsense"), "{message}");
}

#[test]
fn a_forbidden_key_in_a_plugin_sidecar_names_the_section() {
    let tree = tree_with_skill("---\ndescription: d\n---\nB.\n");
    tree.write(
        "plugins/alpha/.claude-plugin/systemprompt.yaml",
        "schema: 1\nplugin:\n  category: ops\n  license: MIT\n",
    );

    let message = expect_err(&tree);

    assert!(
        message.contains("'plugin.license' is not allowed"),
        "{message}"
    );
    assert!(message.contains("second source of truth"), "{message}");
}

#[test]
fn an_unparsable_sidecar_names_the_file() {
    let tree = tree_with_skill("---\ndescription: d\n---\nB.\n");
    tree.write(
        "plugins/alpha/.claude-plugin/systemprompt.yaml",
        "schema: 1\nplugin: [unclosed\n",
    );

    let message = expect_err(&tree);

    assert!(message.contains("systemprompt.yaml"), "{message}");
}

#[test]
fn a_sidecar_with_no_body_section_takes_every_default() {
    let tree = tree_with_skill("---\ndescription: d\n---\nB.\n");
    tree.write(
        "plugins/alpha/.claude-plugin/systemprompt.yaml",
        "schema: 1\n",
    );

    let (dest, report) = expect_ok(&tree);
    let plugin = plugin_yaml(dest.path(), "alpha")["plugin"].clone();

    assert_eq!(plugin["enabled"].as_bool(), Some(true));
    assert_eq!(plugin["name"].as_str(), Some("alpha"));
    assert_eq!(plugin["category"].as_str(), Some("ops"));
    assert!(
        plugin["mcp_servers"]["include"]
            .as_sequence()
            .is_none_or(Vec::is_empty)
    );
    assert!(
        !report
            .warnings
            .iter()
            .any(|w| matches!(w, ImportWarning::MissingCategory { .. }))
    );
}

#[test]
fn a_blank_sidecar_title_falls_back_to_the_plugin_id() {
    let tree = tree_with_skill("---\ndescription: d\n---\nB.\n");
    tree.write(
        "plugins/alpha/.claude-plugin/systemprompt.yaml",
        "schema: 1\nplugin:\n  category: ops\n  title: '  '\n",
    );

    let (dest, _report) = expect_ok(&tree);

    assert_eq!(
        plugin_yaml(dest.path(), "alpha")["plugin"]["name"].as_str(),
        Some("alpha")
    );
}

#[test]
fn a_disabled_plugin_sidecar_disables_the_imported_plugin() {
    let tree = tree_with_skill("---\ndescription: d\n---\nB.\n");
    tree.write(
        "plugins/alpha/.claude-plugin/systemprompt.yaml",
        "schema: 1\nplugin:\n  category: ops\n  enabled: false\n",
    );

    let (dest, _report) = expect_ok(&tree);

    assert_eq!(
        plugin_yaml(dest.path(), "alpha")["plugin"]["enabled"].as_bool(),
        Some(false)
    );
}

#[test]
fn a_forbidden_key_in_a_marketplace_sidecar_names_the_section() {
    let tree = Tree::new();
    tree.write(".claude-plugin/marketplace.json", &marketplace_json(""));
    tree.write(
        ".claude-plugin/systemprompt.yaml",
        "schema: 1\nmarketplace:\n  keywords: [a]\n",
    );

    let message = expect_err(&tree);

    assert!(
        message.contains("'marketplace.keywords' is not allowed"),
        "{message}"
    );
}

#[test]
fn a_marketplace_sidecar_supplies_the_title_visibility_and_enabled_flag() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        r#"{"name":"acme","metadata":{"description":"desc","version":"2.0.0"},"plugins":[]}"#,
    );
    tree.write(
        ".claude-plugin/systemprompt.yaml",
        "schema: 1\nmarketplace:\n  title: Acme Field\n  visibility: private\n  enabled: false\n",
    );

    let (dest, report) = expect_ok(&tree);
    let text = std::fs::read_to_string(dest.path().join("marketplaces/acme/config.yaml"))
        .expect("marketplace config");
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("valid yaml");
    let mp = doc["marketplace"].clone();

    assert_eq!(report.marketplaces.len(), 1);
    assert_eq!(mp["name"].as_str(), Some("Acme Field"));
    assert_eq!(mp["visibility"].as_str(), Some("private"));
    assert_eq!(mp["enabled"].as_bool(), Some(false));
    assert_eq!(mp["version"].as_str(), Some("2.0.0"));
    assert_eq!(mp["description"].as_str(), Some("desc"));
}

#[test]
fn a_marketplace_with_no_version_gets_the_importer_default() {
    let tree = Tree::new();
    tree.write(
        ".claude-plugin/marketplace.json",
        r#"{"name":"acme","metadata":{"version":"  "},"plugins":[]}"#,
    );

    let (dest, _report) = expect_ok(&tree);
    let text = std::fs::read_to_string(dest.path().join("marketplaces/acme/config.yaml"))
        .expect("marketplace config");

    assert!(text.contains("version: 0.1.0"), "{text}");
    assert!(text.contains("license: proprietary"), "{text}");
}
