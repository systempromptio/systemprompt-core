use std::fs;
use std::path::Path;

use systemprompt_bridge::gui::server_marketplace::{mark_shared_mcp, plugin_children};

fn write_plugin(root: &Path, name: &str, mcp_servers: &[&str]) -> std::path::PathBuf {
    let dir = root.join(name);
    fs::create_dir_all(dir.join("skills").join("draft_email")).unwrap();
    fs::write(
        dir.join("skills").join("draft_email").join("SKILL.md"),
        "---\nname: Draft Email\ndescription: Draft things\n---\nBody\n",
    )
    .unwrap();
    fs::create_dir_all(dir.join("agents")).unwrap();
    fs::write(
        dir.join("agents").join("helper.md"),
        "---\nname: Helper Agent\n---\nBody\n",
    )
    .unwrap();
    let servers: Vec<String> = mcp_servers
        .iter()
        .map(|s| format!("\"{s}\": {{\"url\": \"https://example.com/mcp\"}}"))
        .collect();
    fs::write(
        dir.join(".mcp.json"),
        format!("{{\"mcpServers\": {{{}}}}}", servers.join(",")),
    )
    .unwrap();
    dir
}

#[test]
fn plugin_children_scans_skills_agents_and_mcp() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = write_plugin(tmp.path(), "astound-salesforce-accounts", &["salesforce"]);

    let children = plugin_children(&dir);

    let skill = children.iter().find(|c| c.kind == "skills").unwrap();
    assert_eq!(skill.id, "draft_email");
    assert_eq!(skill.name, "Draft Email");
    let agent = children.iter().find(|c| c.kind == "agents").unwrap();
    assert_eq!(agent.id, "helper");
    assert_eq!(agent.name, "Helper Agent");
    let mcp = children.iter().find(|c| c.kind == "mcp").unwrap();
    assert_eq!(mcp.id, "salesforce");
    assert!(!mcp.shared);
}

#[test]
fn plugin_children_empty_for_bare_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("empty-plugin");
    fs::create_dir_all(&dir).unwrap();
    assert!(plugin_children(&dir).is_empty());
}

#[test]
fn mark_shared_mcp_flags_servers_used_by_multiple_plugins() {
    let tmp = tempfile::tempdir().unwrap();
    let a = write_plugin(tmp.path(), "plugin-a", &["salesforce", "only-a"]);
    let b = write_plugin(tmp.path(), "plugin-b", &["salesforce"]);

    let mut sets = vec![plugin_children(&a), plugin_children(&b)];
    mark_shared_mcp(&mut sets);

    let shared: Vec<(&str, bool)> = sets
        .iter()
        .flatten()
        .filter(|c| c.kind == "mcp")
        .map(|c| (c.id.as_str(), c.shared))
        .collect();
    assert!(shared.contains(&("salesforce", true)));
    assert!(shared.contains(&("only-a", false)));
    assert_eq!(
        shared.iter().filter(|(id, _)| *id == "salesforce").count(),
        2
    );
}
