use std::fs;

use serde_json::{Value, json};
use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_device::protocol::names::sha256_hex;
use systemprompt_harness::projection::{
    BundleFile, BundleFiles, Projection, ProjectionError, wanted_bundle_files,
};
use systemprompt_identifiers::PluginId;
use systemprompt_models::bridge::host::HostKind;

use crate::projection::fixtures;

const THEME: &str = "{ \"base\": {} }\n";

fn plugin(id: &str, files: &[(&str, &str)]) -> Value {
    let files: Vec<Value> = files
        .iter()
        .map(|(path, text)| json!({ "path": path, "sha256": sha256_hex(text.as_bytes()) }))
        .collect();
    json!({ "id": id, "files": files })
}

fn delivered(into: &mut BundleFiles, id: &str, files: &[(&str, &str)]) {
    for (path, text) in files {
        into.insert(&PluginId::new(id), *path, text.as_bytes().to_vec());
    }
}

fn with_plugins(plugins: &[Value], harness: Option<Value>) -> Manifest {
    let marketplace = harness.map_or_else(
        || json!({ "id": "m" }),
        |settings| json!({ "id": "m", "harness": settings }),
    );
    fixtures::manifest(json!({ "plugins": plugins, "marketplaces": [marketplace] }))
}

fn texts(files: &[BundleFile]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|file| {
            (
                file.path.clone(),
                String::from_utf8_lossy(&file.bytes).into_owned(),
            )
        })
        .collect()
}

#[test]
fn wanted_lists_agents_and_pinned_themes_by_plugin_id() {
    let theme_path = format!("hosts/{}/themes/astound.json", HostKind::Harness.as_str());
    let manifest = with_plugins(
        &[
            plugin(
                "zeta",
                &[
                    ("agents/helper.md", "# Helper"),
                    ("agents/nested/deep.md", "# Deep"),
                    ("agents/notes.txt", "x"),
                    ("agents/.md", "x"),
                    ("hosts/other/themes/astound.json", THEME),
                ],
            ),
            plugin(
                "commons",
                &[
                    (".claude-plugin/plugin.json", "{}"),
                    (theme_path.as_str(), THEME),
                ],
            ),
            plugin("../escape", &[("agents/evil.md", "x")]),
        ],
        Some(json!({ "themes": [{ "name": "astound", "sha": sha256_hex(THEME.as_bytes()) }] })),
    );
    let listed: Vec<String> = wanted_bundle_files(&manifest, HostKind::Harness)
        .into_iter()
        .map(|(plugin, path, _)| format!("{}/{path}", plugin.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            format!("commons/{theme_path}"),
            "zeta/agents/helper.md".to_owned()
        ]
    );
}

#[test]
fn a_theme_is_taken_only_at_the_pinned_digest() {
    let theme_path = format!("hosts/{}/themes/astound.json", HostKind::Harness.as_str());
    let files = [(theme_path.as_str(), THEME)];
    let pinned = |harness: Option<Value>| {
        let manifest = with_plugins(&[plugin("p", &files)], harness);
        let mut bundle = BundleFiles::new();
        delivered(&mut bundle, "p", &files);
        fixtures::projected(&manifest, &bundle)
            .bundle_files
            .into_iter()
            .map(|file| file.path)
            .collect::<Vec<_>>()
    };
    assert_eq!(pinned(None), Vec::<String>::new());
    let other = sha256_hex(b"another build");
    assert_eq!(
        pinned(Some(
            json!({ "themes": [{ "name": "astound", "sha": other }] })
        )),
        Vec::<String>::new()
    );
    let right = sha256_hex(THEME.as_bytes());
    assert_eq!(
        pinned(Some(
            json!({ "themes": [{ "name": "astound", "sha": right }] })
        )),
        ["themes/astound.json"]
    );
}

#[test]
fn agents_become_subagents() {
    let files = [
        (
            "agents/reviewer.md",
            "---\ndescription: Reviews\nmode: primary\n---\n\nReview it.\n",
        ),
        ("agents/plain.md", "\u{feff}Just a prompt.\n"),
    ];
    let manifest = with_plugins(&[plugin("p", &files)], None);
    let mut bundle = BundleFiles::new();
    delivered(&mut bundle, "p", &files);
    let projection = fixtures::projected(&manifest, &bundle);
    assert_eq!(projection.warnings, Vec::<String>::new());
    assert_eq!(projection.counts.agents, 2);
    assert_eq!(
        texts(&projection.bundle_files),
        [
            (
                "agents/plain.md".to_owned(),
                "---\nmode: subagent\n---\n\nJust a prompt.\n".to_owned()
            ),
            (
                "agents/reviewer.md".to_owned(),
                "---\nmode: subagent\ndescription: Reviews\n---\n\nReview it.\n".to_owned()
            ),
        ]
    );
}

#[test]
fn a_digest_mismatch_is_skipped_and_the_next_plugin_takes_the_name() {
    let manifest = with_plugins(
        &[
            plugin("b-second", &[("agents/helper.md", "# Second")]),
            plugin("a-first", &[("agents/helper.md", "# Signed")]),
        ],
        None,
    );
    let mut bundle = BundleFiles::new();
    delivered(
        &mut bundle,
        "a-first",
        &[("agents/helper.md", "# Tampered")],
    );
    delivered(&mut bundle, "b-second", &[("agents/helper.md", "# Second")]);
    let projection = fixtures::projected(&manifest, &bundle);
    assert_eq!(
        texts(&projection.bundle_files),
        [(
            "agents/helper.md".to_owned(),
            "---\nmode: subagent\n---\n\n# Second".to_owned()
        )]
    );
    assert_eq!(projection.warnings.len(), 1);
    assert!(projection.warnings[0].starts_with("agents/helper.md of plugin a-first not projected"));
}

#[test]
fn the_first_plugin_by_id_wins_and_a_missing_file_is_reported() {
    let files = [("agents/helper.md", "# Helper")];
    let manifest = with_plugins(
        &[
            plugin("b", &files),
            plugin("a", &files),
            plugin("c", &[("agents/other.md", "# Other")]),
        ],
        None,
    );
    let mut bundle = BundleFiles::new();
    delivered(&mut bundle, "b", &files);
    delivered(&mut bundle, "a", &files);
    let projection = fixtures::projected(&manifest, &bundle);
    let paths: Vec<&str> = projection
        .bundle_files
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(paths, ["agents/helper.md"]);
    assert_eq!(
        projection.warnings,
        ["agents/other.md of plugin c not projected: not delivered"]
    );
}

#[test]
fn owned_paths_are_one_file_deep() {
    let root = tempfile::tempdir().unwrap();
    let applier = fixtures::applier(root.path());
    let with_file = |path: &str| {
        let mut projection: Projection =
            fixtures::projected(&fixtures::manifest(json!({})), &BundleFiles::new());
        projection.bundle_files.push(BundleFile {
            path: path.to_owned(),
            bytes: b"x".to_vec(),
        });
        projection
    };
    for good in ["agents/a.md", "agents/A.MD", "themes/astound.json"] {
        assert!(applier.apply(&with_file(good)).is_ok(), "{good}");
        assert_eq!(fs::read(applier.config_dir().join(good)).unwrap(), b"x");
    }
    let bad_paths = [
        "settings.json",
        "harness.json",
        "../outside.md",
        "agents/../x.md",
        "agents/x/y.md",
        "agents/.md",
        "themes/.json",
        "themes/a/b.json",
        "/etc/hosts",
    ];
    for bad in bad_paths {
        assert!(
            matches!(
                applier.apply(&with_file(bad)),
                Err(ProjectionError::Unowned { .. })
            ),
            "{bad}"
        );
    }
}
