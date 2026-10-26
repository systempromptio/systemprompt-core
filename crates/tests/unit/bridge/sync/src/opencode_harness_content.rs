//! `OpenCode` harness content: the rules block of `AGENTS.md`, bundle agents
//! and plugins under the owned ledger, and the managed (MDM) provider tier.

use std::fs;
use std::path::Path;

use systemprompt_bridge::gateway::manifest::{PluginEntry, PluginFile, RuleEntry};
use systemprompt_bridge::ids::{PluginId, Sha256Digest};
use systemprompt_bridge::integration::opencode::{agents_block, as_subagent, splice_block};

use crate::opencode_managed_resources::{apply, manifest_with, mcp, with_sandbox};

fn rule(id: &str, hosts: &[&str], body: &str) -> RuleEntry {
    RuleEntry {
        id: systemprompt_identifiers::MarketplaceRuleId::new(id),
        name: systemprompt_identifiers::RuleName::new(id),
        description: String::new(),
        file_path: format!("rules/{id}/index.md"),
        tags: Vec::new(),
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
        instructions: body.to_owned(),
        hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
        plugins: Vec::new(),
        publication: None,
    }
}

fn sha(bytes: &[u8]) -> Sha256Digest {
    use sha2::Digest as _;
    Sha256Digest::try_new(
        sha2::Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
    )
    .unwrap()
}

fn bundle(root: &Path, id: &str, files: &[(&str, &[u8])], corrupt: &[&str]) -> PluginEntry {
    let mut entries = Vec::new();
    for (path, bytes) in files {
        let target = root.join(id).join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        let written: &[u8] = if corrupt.contains(path) {
            b"tampered"
        } else {
            bytes
        };
        fs::write(&target, written).unwrap();
        entries.push(PluginFile {
            path: (*path).to_owned(),
            sha256: sha(bytes),
            size: bytes.len() as u64,
        });
    }
    PluginEntry {
        id: PluginId::try_new(id).unwrap(),
        version: "1.0.0".into(),
        sha256: Sha256Digest::try_new("0".repeat(64)).unwrap(),
        files: entries,
        hooks: Default::default(),
        publication: None,
    }
}

#[test]
fn the_rules_block_lists_only_opencode_rules_ordered_by_id() {
    let block = agents_block(&[
        rule("zeta", &["opencode-cli"], "Last."),
        rule("alpha", &["opencode"], "First."),
        rule("claude-only", &["claude-code"], "Never here."),
        rule("everywhere", &[], "Not named, not projected."),
    ])
    .expect("a block");
    assert_eq!(
        block,
        "<!-- systemprompt:begin (managed by the bridge; edits are replaced) -->\n\n## \
         alpha\n\nFirst.\n\n## zeta\n\nLast.\n\n<!-- systemprompt:end -->\n"
    );
    assert!(agents_block(&[rule("c", &["codex"], "x")]).is_none());
}

#[test]
fn splicing_replaces_only_the_block_and_keeps_user_text() {
    let block = agents_block(&[rule("a", &["opencode"], "Rule.")]).unwrap();
    let first = splice_block("# My notes\n", Some(&block));
    assert_eq!(first, format!("# My notes\n\n{block}"));
    let replaced = splice_block(&first, Some(&block.replace("Rule.", "Changed.")));
    assert!(replaced.starts_with("# My notes\n\n") && replaced.contains("Changed."));
    assert!(!replaced.contains("Rule."));
    assert_eq!(splice_block(&first, None), "# My notes\n");
    assert_eq!(splice_block(&block, None), "");
}

#[test]
fn a_bundle_agent_becomes_a_subagent() {
    assert_eq!(
        as_subagent("---\ndescription: Reviews code\nmode: primary\n---\nReview it.\n"),
        "---\nmode: subagent\ndescription: Reviews code\n---\nReview it.\n"
    );
    assert_eq!(
        as_subagent("Just a body.\n"),
        "---\nmode: subagent\n---\n\nJust a body.\n"
    );
}

#[test]
fn sync_projects_rules_agents_and_bundle_plugins_and_prunes_what_it_dropped() {
    with_sandbox(|sb| {
        let opencode = sb.config.parent().unwrap().to_path_buf();
        fs::create_dir_all(&opencode).unwrap();
        fs::write(opencode.join("AGENTS.md"), "# Mine\n").unwrap();
        let plugins = sb.skills.clone();
        let mut m = manifest_with(vec![], vec![]);
        m.rules = vec![rule("security", &["opencode"], "Never paste secrets.")];
        m.plugins = vec![bundle(
            &plugins,
            "acme",
            &[
                ("agents/reviewer.md", b"Review carefully.\n"),
                ("hosts/opencode/plugins/audit.js", b"export default {}\n"),
                ("hosts/opencode/plugins/tampered.ts", b"export default {}\n"),
                ("skills/x/SKILL.md", b"ignored\n"),
            ],
            &["hosts/opencode/plugins/tampered.ts"],
        )];
        apply(&m, &plugins).unwrap();
        let agents_md = fs::read_to_string(opencode.join("AGENTS.md")).unwrap();
        assert!(agents_md.starts_with("# Mine\n\n") && agents_md.contains("Never paste secrets."));
        let agent = fs::read_to_string(opencode.join("agents/reviewer.md")).unwrap();
        assert!(agent.starts_with("---\nmode: subagent\n"), "{agent}");
        assert!(opencode.join("plugins/acme-audit.js").is_file());
        assert!(
            !opencode.join("plugins/acme-tampered.ts").exists(),
            "a file that fails its digest is never projected"
        );

        let mut dropped = manifest_with(vec![], vec![]);
        dropped.plugins = vec![];
        apply(&dropped, &plugins).unwrap();
        assert_eq!(
            fs::read_to_string(opencode.join("AGENTS.md")).unwrap(),
            "# Mine\n"
        );
        assert!(!opencode.join("agents/reviewer.md").exists());
        assert!(!opencode.join("plugins/acme-audit.js").exists());
    });
}

#[test]
fn an_mdm_provider_tier_gets_content_but_no_bridge_mcp_servers() {
    with_sandbox(|sb| {
        let managed = sb
            .config
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("managed");
        fs::create_dir_all(&managed).unwrap();
        fs::write(
            managed.join("opencode.json"),
            r#"{ "providers": { "systemprompt": {} } }"#,
        )
        .unwrap();
        let mut m = manifest_with(vec![], vec![mcp("primary")]);
        m.rules = vec![rule("security", &["opencode"], "Never paste secrets.")];
        apply(&m, &sb.skills).unwrap();
        let config = fs::read_to_string(&sb.config).unwrap_or_default();
        assert!(!config.contains("primary"), "{config}");
        let agents_md = sb.config.with_file_name("AGENTS.md");
        assert!(
            fs::read_to_string(agents_md)
                .unwrap()
                .contains("Never paste secrets.")
        );
    });
}
