//! Filesystem error branches and catalogue edge paths of the loader:
//! unreadable catalog entries, directory-shaped includes, duplicate
//! marketplaces across includes, deprecated `card.skills` warnings, skill
//! instruction includes, profile save/list failures, nested extension
//! groups, and `ConfigWriter` lookups where the filename differs from the
//! agent name.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use systemprompt_loader::{ConfigLoader, ConfigWriter, ExtensionLoader, ProfileLoader};
use tempfile::TempDir;

fn base_config() -> &'static str {
    r#"
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#
}

fn chmod(path: &Path, mode: u32) {
    let mut perms = std::fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(mode);
    std::fs::set_permissions(path, perms).expect("set permissions");
}

fn perms_are_enforced(path: &Path) -> bool {
    std::fs::read_to_string(path).is_err() || std::fs::read_dir(path).is_err()
}

#[test]
fn unreadable_skill_config_surfaces_io_error() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("config dir");
    let config_path = config_dir.join("config.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let skill_dir = temp.path().join("skills/broken-skill");
    std::fs::create_dir_all(&skill_dir).expect("skill dir");
    let skill_config = skill_dir.join("config.yaml");
    std::fs::write(&skill_config, "id: broken-skill\nname: b\n").expect("write skill");
    chmod(&skill_config, 0o000);
    if !perms_are_enforced(&skill_config) {
        chmod(&skill_config, 0o644);
        return;
    }

    let err =
        ConfigLoader::load_from_path(&config_path).expect_err("unreadable skill config must error");
    chmod(&skill_config, 0o644);
    assert!(format!("{err:#}").contains("broken-skill"), "got {err:#}");
}

#[test]
fn unreadable_skills_catalog_dir_surfaces_io_error() {
    let temp = TempDir::new().expect("tempdir");
    let config_dir = temp.path().join("config");
    std::fs::create_dir_all(&config_dir).expect("config dir");
    let config_path = config_dir.join("config.yaml");
    std::fs::write(&config_path, base_config()).expect("write config");

    let skills_dir = temp.path().join("skills");
    std::fs::create_dir_all(&skills_dir).expect("skills dir");
    chmod(&skills_dir, 0o000);
    if !perms_are_enforced(&skills_dir) {
        chmod(&skills_dir, 0o755);
        return;
    }

    let err =
        ConfigLoader::load_from_path(&config_path).expect_err("unreadable catalog must error");
    chmod(&skills_dir, 0o755);
    assert!(format!("{err:#}").contains("skills"), "got {err:#}");
}

#[test]
fn include_pointing_at_directory_is_io_error() {
    let temp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("dir-include")).expect("include dir");
    let config_path = temp.path().join("config.yaml");
    let main = r#"
includes:
  - dir-include
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let err = ConfigLoader::load_from_content(main, &config_path)
        .expect_err("directory include must fail to read");
    assert!(format!("{err:#}").contains("dir-include"), "got {err:#}");
}

#[test]
fn duplicate_marketplace_across_includes_is_rejected() {
    let temp = TempDir::new().expect("tempdir");
    let marketplace = r#"
marketplaces:
  dup-market:
    name: Dup
    description: duplicated marketplace
    version: 1.0.0
    enabled: true
    author:
      name: fixture
      email: fixture@example.com
    keywords: []
    license: MIT
"#;
    std::fs::write(temp.path().join("m1.yaml"), marketplace).expect("write m1");
    std::fs::write(temp.path().join("m2.yaml"), marketplace).expect("write m2");
    let main = r#"
includes:
  - m1.yaml
  - m2.yaml
agents: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let err = ConfigLoader::load_from_content(main, &temp.path().join("config.yaml"))
        .expect_err("duplicate marketplace must be rejected");
    assert!(format!("{err:#}").contains("dup-market"), "got {err:#}");
}

#[test]
fn authored_card_skills_load_and_warn() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("config.yaml");
    let main = r#"
agents:
  legacy:
    name: legacy
    port: 4000
    endpoint: http://localhost:4000/legacy
    enabled: true
    card:
      protocolVersion: "0.2.3"
      displayName: "Legacy Agent"
      description: "Authors card.skills directly"
      version: "1.0.0"
      preferredTransport: JSONRPC
      capabilities:
        streaming: true
      defaultInputModes: [text/plain]
      defaultOutputModes: [text/plain]
      skills:
        - id: legacy-skill
          name: Legacy Skill
          description: deprecated authoring path
      supportsAuthenticatedExtendedCard: false
    metadata: {}
mcp_servers: {}
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let config =
        ConfigLoader::load_from_content(main, &config_path).expect("legacy card.skills loads");
    assert_eq!(config.agents["legacy"].card.skills.len(), 1);
}

#[test]
fn skill_instructions_include_is_resolved_inline() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("config.yaml");
    std::fs::write(temp.path().join("instructions.md"), "Do the thing.").expect("write include");
    let main = r#"
agents: {}
mcp_servers: {}
skills:
  skills:
    guided:
      id: guided
      name: Guided
      description: skill with included instructions
      enabled: true
      instructions: "!include instructions.md"
settings:
  agent_port_range: [4000, 4999]
  mcp_port_range: [5000, 5999]
ai:
  default_provider: anthropic
"#;

    let config = ConfigLoader::load_from_content(main, &config_path).expect("loads");
    let instructions = config.skills.skills["guided"]
        .instructions
        .as_ref()
        .expect("instructions present");
    assert_eq!(instructions.as_inline(), Some("Do the thing."));
}

#[test]
fn get_includes_reports_yaml_error() {
    let temp = TempDir::new().expect("tempdir");
    let config_path = temp.path().join("config.yaml");
    std::fs::write(&config_path, ": not : valid : yaml :").expect("write");

    let loader = ConfigLoader::new(config_path.clone());
    let err = loader
        .get_includes()
        .expect_err("invalid yaml must fail include listing");
    assert!(format!("{err:#}").contains("config.yaml"), "got {err:#}");
}

#[test]
fn profile_list_unreadable_dir_returns_empty() {
    let temp = TempDir::new().expect("tempdir");
    let profiles_dir = temp.path().join("profiles");
    std::fs::create_dir_all(&profiles_dir).expect("profiles dir");
    chmod(&profiles_dir, 0o000);
    if !perms_are_enforced(&profiles_dir) {
        chmod(&profiles_dir, 0o755);
        return;
    }

    let listed = ProfileLoader::list_available(temp.path());
    chmod(&profiles_dir, 0o755);
    assert!(listed.is_empty());
}

#[test]
fn extension_discovery_scans_nested_groups_and_skips_bad_manifests() {
    let temp = TempDir::new().expect("tempdir");
    let nested = temp.path().join("extensions/org-group/nested-ext");
    std::fs::create_dir_all(&nested).expect("nested ext dir");
    std::fs::write(
        nested.join("manifest.yaml"),
        "extension:\n  type: mcp\n  name: nested-ext\n  binary: nested-bin\n  enabled: true\n",
    )
    .expect("write nested manifest");

    let broken = temp.path().join("extensions/broken-ext");
    std::fs::create_dir_all(&broken).expect("broken ext dir");
    std::fs::write(broken.join("manifest.yaml"), ": nope : nope :").expect("write bad manifest");

    let discovered = ExtensionLoader::discover(temp.path());
    let names: Vec<&str> = discovered
        .iter()
        .map(|e| e.manifest.extension.name.as_str())
        .collect();
    assert_eq!(names, vec!["nested-ext"]);
}

#[test]
fn find_agent_file_scans_for_mismatched_filenames() {
    let temp = TempDir::new().expect("tempdir");
    let agents_dir = temp.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("agents dir");
    std::fs::write(
        agents_dir.join("renamed-file.yaml"),
        "agents:\n  hidden-agent:\n    name: hidden-agent\n    port: 4000\n    endpoint: \
         http://localhost:4000/hidden\n    enabled: true\n    card:\n      protocolVersion: \
         \"0.2.3\"\n      displayName: Hidden\n      description: d\n      version: \"1.0.0\"\n    \
         metadata: {}\n",
    )
    .expect("write agent file");

    let found = ConfigWriter::find_agent_file("hidden-agent", temp.path())
        .expect("scan succeeds")
        .expect("agent located despite filename mismatch");
    assert!(found.ends_with("agents/renamed-file.yaml"));

    let missing = ConfigWriter::find_agent_file("absent-agent", temp.path()).expect("scan");
    assert!(missing.is_none());
}

#[test]
fn find_agent_file_unreadable_candidate_is_io_error() {
    let temp = TempDir::new().expect("tempdir");
    let agents_dir = temp.path().join("agents");
    std::fs::create_dir_all(&agents_dir).expect("agents dir");
    let file = agents_dir.join("locked.yaml");
    std::fs::write(&file, "agents: {}\n").expect("write");
    chmod(&file, 0o000);
    if !perms_are_enforced(&file) {
        chmod(&file, 0o644);
        return;
    }

    let err = ConfigWriter::find_agent_file("locked", temp.path())
        .expect_err("unreadable candidate must surface Io");
    chmod(&file, 0o644);
    assert!(format!("{err:#}").contains("locked"), "got {err:#}");
}
