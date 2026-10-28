//! Layered profile loading, derived fields, overlay writes and the
//! committed-profile temp-path gate.

use std::path::{Path, PathBuf};

use serde_yaml::Value;
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::derived::internal_url_for;
use systemprompt_manifest::profile::{
    AuthzMode, ProfileError, committed_source_errors, merge_yaml,
};

const BASE: &str = r"
name: base
display_name: Base
site:
  name: Site
database:
  type: postgres
server:
  host: 127.0.0.1
  port: 8080
  external_url: http://localhost:8080
  extra_cors_origins:
    - http://localhost:5173
paths:
  system: .
  services: services
  bin: bin
security:
  jwt_access_token_expiration: 3600
  jwt_refresh_token_expiration: 86400
  jwt_audiences: [api]
rate_limits: {}
system_admin:
  username: admin
governance:
  authz:
    hook:
      mode: webhook
runtime:
  log_level: verbose
";

fn yaml(text: &str) -> Value {
    serde_yaml::from_str(text).unwrap()
}

struct Tree {
    _dir: tempfile::TempDir,
    profiles: PathBuf,
}

impl Tree {
    fn new(base: Option<&str>, env: &str, overlay: Option<&str>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let profiles = dir.path().join("profiles");
        std::fs::create_dir_all(profiles.join(env)).unwrap();
        if let Some(base) = base {
            std::fs::write(profiles.join("base.yaml"), base).unwrap();
        }
        if let Some(overlay) = overlay {
            std::fs::write(profiles.join(env).join("profile.yaml"), overlay).unwrap();
        }
        Self {
            _dir: dir,
            profiles,
        }
    }

    fn overlay(&self, env: &str) -> PathBuf {
        self.profiles.join(env).join("profile.yaml")
    }
}

#[test]
fn merge_overlay_wins_maps_merge_sequences_replace() {
    let merged = merge_yaml(
        yaml("a: {x: 1, y: 2}\nlist: [1, 2]\nkeep: k"),
        yaml("a: {y: 3, z: 4}\nlist: [9]"),
    );
    assert_eq!(merged, yaml("a: {x: 1, y: 3, z: 4}\nlist: [9]\nkeep: k"));
}

#[test]
fn load_layered_applies_overlay_over_base() {
    let tree = Tree::new(
        Some(BASE),
        "prod",
        Some(
            "name: prod\nserver:\n  external_url: https://prod.example.com\n  extra_cors_origins: [https://app.example.com]\nruntime:\n  log_level: quiet\n",
        ),
    );
    let profile = Profile::load_layered(&tree.profiles, "prod").unwrap();
    assert_eq!(profile.name, "prod");
    assert_eq!(profile.display_name, "Base");
    assert_eq!(profile.server.port, 8080);
    assert_eq!(
        profile.server.cors_allowed_origins,
        vec!["https://prod.example.com", "https://app.example.com"]
    );
    assert!(matches!(
        profile.runtime.log_level,
        systemprompt_manifest::LogLevel::Quiet
    ));
}

#[test]
fn missing_base_is_overlay_only() {
    let tree = Tree::new(None, "local", Some(BASE));
    let profile = Profile::load_layered(&tree.profiles, "local").unwrap();
    assert_eq!(profile.name, "base");
}

#[test]
fn missing_overlay_is_an_error() {
    let tree = Tree::new(Some(BASE), "local", None);
    let err = Profile::load_layered(&tree.profiles, "local").unwrap_err();
    assert!(
        matches!(err, ProfileError::MissingOverlay { .. }),
        "{err:?}"
    );
}

#[test]
fn derived_fields_follow_external_url_and_bind() {
    let tree = Tree::new(
        Some(BASE),
        "prod",
        Some(
            "server:\n  host: 0.0.0.0\n  port: 9000\n  external_url: https://api.example.com:8443/\n",
        ),
    );
    let profile = Profile::load_layered(&tree.profiles, "prod").unwrap();
    let server = &profile.server;
    assert_eq!(server.external_url, "https://api.example.com:8443");
    assert_eq!(server.api_server_url, "https://api.example.com:8443");
    assert_eq!(server.api_external_url, "https://api.example.com:8443");
    assert!(server.use_https);
    assert_eq!(
        server.cors_allowed_origins[0],
        "https://api.example.com:8443"
    );
    assert_eq!(profile.security.issuer, "https://api.example.com:8443");
    assert_eq!(server.api_internal_url, "http://localhost:9000");
    let hook = &profile.governance.unwrap().authz.unwrap().hook;
    assert_eq!(hook.mode, AuthzMode::Webhook);
    assert_eq!(
        hook.url.as_deref(),
        Some("http://localhost:9000/api/public/govern/authz")
    );
}

#[test]
fn http_external_url_default_port_origin() {
    let tree = Tree::new(
        Some(BASE),
        "local",
        Some("server:\n  external_url: http://example.test:80\n"),
    );
    let profile = Profile::load_layered(&tree.profiles, "local").unwrap();
    assert!(!profile.server.use_https);
    assert_eq!(
        profile.server.cors_allowed_origins[0],
        "http://example.test"
    );
    assert_eq!(profile.server.api_internal_url, "http://127.0.0.1:8080");
}

#[test]
fn internal_url_brackets_ipv6() {
    assert_eq!(internal_url_for("::1", 80), "http://[::1]:80");
    assert_eq!(internal_url_for("::", 80), "http://localhost:80");
}

#[test]
fn every_derived_field_is_refused_when_stated() {
    for (snippet, field, source) in [
        (
            "server:\n  api_server_url: x\n",
            "server.api_server_url",
            "server.external_url",
        ),
        (
            "server:\n  api_external_url: x\n",
            "server.api_external_url",
            "server.external_url",
        ),
        (
            "server:\n  use_https: true\n",
            "server.use_https",
            "server.external_url",
        ),
        (
            "server:\n  cors_allowed_origins: []\n",
            "server.cors_allowed_origins",
            "server.external_url",
        ),
        (
            "security:\n  jwt_issuer: x\n",
            "security.jwt_issuer",
            "server.external_url",
        ),
        (
            "server:\n  api_internal_url: x\n",
            "server.api_internal_url",
            "server.host and server.port",
        ),
        (
            "governance:\n  authz:\n    hook:\n      mode: webhook\n      url: x\n",
            "governance.authz.hook.url",
            "server.host and server.port",
        ),
    ] {
        let tree = Tree::new(Some(BASE), "env", Some(snippet));
        let err = Profile::load_layered(&tree.profiles, "env").unwrap_err();
        match &err {
            ProfileError::DerivedField {
                field: f,
                source_field: s,
                ..
            } => {
                assert_eq!(f, field);
                assert_eq!(s, source);
            },
            other => panic!("{field}: unexpected {other:?}"),
        }
        let message = err.to_string();
        assert!(
            message.contains(field) && message.contains(source),
            "{message}"
        );
    }
}

#[test]
fn legacy_secrets_profile_file_name_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dev.secrets.profile.yaml");
    std::fs::write(&path, BASE).unwrap();
    let err = Profile::load_from_path(&path).unwrap_err();
    assert!(matches!(err, ProfileError::LegacyShape { .. }), "{err:?}");
    assert!(
        err.to_string()
            .contains("systemprompt admin config migrate")
    );
}

#[test]
fn paths_resolve_against_root() {
    let tree = Tree::new(Some(BASE), "local", Some("paths:\n  root: ../..\n"));
    let profile = Profile::load_layered(&tree.profiles, "local").unwrap();
    let root = tree.profiles.parent().unwrap().canonicalize().unwrap();
    assert_eq!(Path::new(&profile.paths.services), root.join("services"));
    assert_eq!(profile.paths.root.as_deref(), root.to_str());
}

#[test]
fn tenant_id_has_one_home() {
    let tree = Tree::new(Some(BASE), "cloud", Some("cloud:\n  tenant_id: t-123\n"));
    let profile = Profile::load_layered(&tree.profiles, "cloud").unwrap();
    assert_eq!(profile.cloud.unwrap().tenant_id.unwrap().as_str(), "t-123");
    let tree = Tree::new(Some(BASE), "x", Some("tenant_id: t-123\n"));
    assert!(Profile::load_layered(&tree.profiles, "x").is_err());
}

#[test]
fn save_overlay_writes_only_changed_keys_and_never_the_base() {
    let tree = Tree::new(Some(BASE), "local", Some("name: local\n"));
    let path = tree.overlay("local");
    let mut profile = Profile::load_from_path(&path).unwrap();
    profile.server.port = 9999;
    profile.save_overlay(&path, None).unwrap();

    let overlay: Value = serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(overlay, yaml("name: local\nserver:\n  port: 9999\n"));
    assert_eq!(
        std::fs::read_to_string(tree.profiles.join("base.yaml")).unwrap(),
        BASE
    );
    assert_eq!(Profile::load_from_path(&path).unwrap().server.port, 9999);
}

#[test]
fn committed_gate_refuses_absolute_temp_paths() {
    let errors = committed_source_errors("paths:\n  system: /tmp/abc/sys\n  bin: bin\n");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("paths.system"), "{errors:?}");
    assert!(committed_source_errors("paths:\n  system: /var/tmp/x\n").len() == 1);
    assert!(committed_source_errors("paths:\n  system: /srv/app\n").is_empty());
}

#[test]
fn create_overlay_keeps_only_what_differs_from_the_base() {
    let tree = Tree::new(Some(BASE), "local", Some("name: local\n"));
    let mut profile = Profile::load_from_path(&tree.overlay("local")).unwrap();
    profile.server.port = 7777;
    let fresh = tree.profiles.join("staging").join("profile.yaml");
    profile.create_overlay(&fresh, Some("# staging")).unwrap();

    let text = std::fs::read_to_string(&fresh).unwrap();
    assert!(text.starts_with("# staging\n\n"), "{text}");
    let overlay: Value = serde_yaml::from_str(&text).unwrap();
    assert_eq!(overlay["name"], yaml("local"));
    assert_eq!(overlay["server"]["port"], yaml("7777"));
    assert!(overlay["server"].get("host").is_none(), "{text}");
    assert!(overlay["site"].get("name").is_none(), "{text}");
    assert_eq!(Profile::load_from_path(&fresh).unwrap().server.port, 7777);
}

#[test]
fn create_overlay_without_a_base_writes_the_whole_profile() {
    let tree = Tree::new(Some(BASE), "local", Some("name: local\n"));
    let profile = Profile::load_from_path(&tree.overlay("local")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let lone = dir.path().join("nested").join("profile.yaml");
    profile.create_overlay(&lone, None).unwrap();
    let overlay: Value = serde_yaml::from_str(&std::fs::read_to_string(&lone).unwrap()).unwrap();
    assert_eq!(overlay["site"]["name"], yaml("Site"));
}

#[test]
fn apply_changes_clears_removed_keys_by_shape() {
    use systemprompt_manifest::profile::overlay::apply_changes;
    let before = yaml("list: [a]\nmap: { k: v }\nscalar: 1\nkept: x\n");
    let after = yaml("kept: x\n");
    let mut overlay = Value::Null;
    apply_changes(&mut overlay, &before, &after);
    assert_eq!(overlay, yaml("list: []\nmap: {}\nscalar: null\n"));

    let mut replaced = yaml("old: 1\n");
    apply_changes(&mut replaced, &yaml("a: 1\n"), &yaml("[1, 2]"));
    assert_eq!(replaced, yaml("[1, 2]"));

    let mut untouched = yaml("old: 1\n");
    apply_changes(&mut untouched, &yaml("a: 1\n"), &yaml("a: 1\n"));
    assert_eq!(untouched, yaml("old: 1\n"));
}
