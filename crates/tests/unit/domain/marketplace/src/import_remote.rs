//! Marketplace entries that name a git source are fetched and imported like a
//! local plugin; the capture is a fake so no test reaches the network.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use systemprompt_marketplace::managed::{
    AssetFile, CapturedGitSource, GitCaptureRequest, GitSourceCapture, ManagedError, Result,
    RevisionFiles,
};
use systemprompt_marketplace::{
    ImportOptions, ImportReport, ImportWarning, MarketplaceError, import_anthropic_tree_with,
};
use tempfile::TempDir;

const SHA: &str = "efc7d4633dfb8fd05baeb8d96fa17f4bb26de498";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    repository: String,
    reference: String,
    subdirectory: Option<String>,
}

struct FakeCapture {
    commit: String,
    files: Vec<(&'static str, &'static str)>,
    seen: Mutex<Vec<Seen>>,
}

impl FakeCapture {
    fn new(files: Vec<(&'static str, &'static str)>) -> Self {
        Self {
            commit: SHA.to_owned(),
            files,
            seen: Mutex::new(Vec::new()),
        }
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().expect("lock").clone()
    }
}

impl GitSourceCapture for FakeCapture {
    fn capture(&self, request: &GitCaptureRequest<'_>) -> Result<CapturedGitSource> {
        self.seen.lock().expect("lock").push(Seen {
            repository: request.repository.to_owned(),
            reference: request.reference.to_owned(),
            subdirectory: request.subdirectory.map(str::to_owned),
        });
        let files = self
            .files
            .iter()
            .map(|(path, body)| {
                (
                    (*path).to_owned(),
                    AssetFile {
                        bytes: body.as_bytes().to_vec(),
                        media_type: "application/octet-stream".to_owned(),
                        executable: false,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        Ok(CapturedGitSource {
            commit: self.commit.clone(),
            files: RevisionFiles(files),
        })
    }
}

struct Unreachable;

impl GitSourceCapture for Unreachable {
    fn capture(&self, _request: &GitCaptureRequest<'_>) -> Result<CapturedGitSource> {
        Err(ManagedError::Unavailable)
    }
}

fn skill(description: &str) -> &'static str {
    Box::leak(
        format!("---\nname: a-skill\ndescription: {description}\n---\n\nBody.\n").into_boxed_str(),
    )
}

fn kit(plugins: &str) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let manifest = dir.path().join(".claude-plugin/marketplace.json");
    std::fs::create_dir_all(manifest.parent().expect("parent")).expect("mkdir");
    std::fs::write(
        manifest,
        format!(r#"{{"name":"acme","owner":{{"name":"Acme"}},"plugins":[{plugins}]}}"#),
    )
    .expect("write manifest");
    dir
}

fn import(
    tree: &TempDir,
    capture: &dyn GitSourceCapture,
    strict: bool,
) -> std::result::Result<(TempDir, ImportReport), MarketplaceError> {
    let dest = TempDir::new().expect("tempdir");
    let opts = ImportOptions {
        strict,
        ..ImportOptions::default()
    };
    let report = import_anthropic_tree_with(tree.path(), dest.path(), &opts, capture)?;
    Ok((dest, report))
}

fn plugin_yaml(dest: &Path, id: &str) -> serde_yaml::Value {
    let text = std::fs::read_to_string(dest.join("plugins").join(id).join("config.yaml"))
        .expect("plugin config written");
    serde_yaml::from_str(&text).expect("valid yaml")
}

fn git_subdir(name: &str, extra: &str) -> String {
    format!(
        r#"{{"name":"{name}","category":"development","strict":false,{extra}"source":{{"source":"git-subdir","url":"acme/tools","path":"skills/{name}","ref":"v1","sha":"{SHA}"}}}}"#
    )
}

#[test]
fn a_git_subdir_plugin_is_fetched_at_its_pin_and_imported() {
    let tree = kit(&git_subdir("b2c", ""));
    let capture = FakeCapture::new(vec![(
        "skills/b2c-controllers/SKILL.md",
        skill("Controllers."),
    )]);

    let (dest, report) = import(&tree, &capture, true).expect("strict import succeeds");

    assert_eq!(
        capture.seen(),
        vec![Seen {
            repository: "https://github.com/acme/tools.git".to_owned(),
            reference: SHA.to_owned(),
            subdirectory: Some("skills/b2c".to_owned()),
        }]
    );
    assert_eq!(report.plugins.len(), 1);
    assert_eq!(report.skills, vec!["b2c_controllers".to_owned()]);
    assert_eq!(
        report.upstream,
        vec![format!(
            "b2c https://github.com/acme/tools.git/skills/b2c@{SHA}"
        )]
    );
    assert!(
        dest.path()
            .join("skills/b2c_controllers/SKILL.md")
            .is_file()
    );
    assert_eq!(plugin_yaml(dest.path(), "b2c")["plugin"]["id"], "b2c");
}

#[test]
fn a_strict_false_entry_is_the_manifest_when_the_upstream_has_none() {
    let tree = kit(&git_subdir(
        "b2c",
        r#""description":"Upstream skills.","version":"1.10.0","#,
    ));
    let capture = FakeCapture::new(vec![
        ("plugin.json", r#"{"name":"not-claude"}"#),
        ("skills/one/SKILL.md", skill("One.")),
    ]);

    let (dest, _report) = import(&tree, &capture, true).expect("import succeeds");

    let plugin = plugin_yaml(dest.path(), "b2c");
    assert_eq!(plugin["plugin"]["description"], "Upstream skills.");
    assert_eq!(plugin["plugin"]["version"], "1.10.0");
}

#[test]
fn a_skills_override_of_the_root_reads_skill_folders_placed_there() {
    let tree = kit(&git_subdir("playwright", r#""skills":["./"],"#));
    let capture = FakeCapture::new(vec![("playwright-cli/SKILL.md", skill("Browser."))]);

    let (_dest, report) = import(&tree, &capture, true).expect("import succeeds");

    assert_eq!(report.skills, vec!["playwright_cli".to_owned()]);
}

#[test]
fn a_skills_override_that_leaves_the_plugin_is_refused() {
    let tree = kit(&git_subdir("escape", r#""skills":["../elsewhere"],"#));
    let capture = FakeCapture::new(vec![("skills/one/SKILL.md", skill("One."))]);

    let err = import(&tree, &capture, false).expect_err("escape refused");

    assert!(
        err.to_string().contains("must stay inside the plugin"),
        "{err}"
    );
}

#[test]
fn a_github_source_fetches_the_whole_repository() {
    let tree = kit(&format!(
        r#"{{"name":"whole","category":"development","strict":false,"source":{{"source":"github","repo":"acme/whole","sha":"{SHA}"}}}}"#
    ));
    let capture = FakeCapture::new(vec![("skills/one/SKILL.md", skill("One."))]);

    import(&tree, &capture, true).expect("import succeeds");

    assert_eq!(capture.seen()[0].subdirectory, None);
    assert_eq!(
        capture.seen()[0].repository,
        "https://github.com/acme/whole.git"
    );
}

#[test]
fn an_unpinned_git_source_is_imported_from_its_ref_but_refused_under_strict() {
    let plugins = r#"{"name":"b2c","category":"development","strict":false,"source":{"source":"git-subdir","url":"acme/tools","path":"skills/b2c","ref":"v1"}}"#;
    let capture = FakeCapture::new(vec![("skills/one/SKILL.md", skill("One."))]);

    let (_dest, report) = import(&kit(plugins), &capture, false).expect("lenient import succeeds");
    assert_eq!(capture.seen()[0].reference, "v1");
    assert!(
        report
            .warnings
            .contains(&ImportWarning::RemotePluginUnpinned {
                plugin: "b2c".to_owned()
            })
    );

    let err = import(&kit(plugins), &capture, true).expect_err("strict refuses");
    assert!(err.to_string().contains("without a `sha`"), "{err}");
}

#[test]
fn an_upstream_that_answers_another_commit_is_refused() {
    let tree = kit(&git_subdir("b2c", ""));
    let mut capture = FakeCapture::new(vec![("skills/one/SKILL.md", skill("One."))]);
    capture.commit = "0".repeat(40);

    let err = import(&tree, &capture, false).expect_err("mismatch refused");

    assert!(err.to_string().contains(&format!("for pin {SHA}")), "{err}");
}

#[test]
fn a_fetch_failure_names_the_plugin_and_its_pin() {
    let tree = kit(&git_subdir("b2c", ""));

    let err = import(&tree, &Unreachable, false).expect_err("fetch failure surfaces");

    let text = err.to_string();
    assert!(text.contains("plugin 'b2c'"), "{text}");
    assert!(text.contains(SHA), "{text}");
}

#[test]
fn an_npm_source_is_skipped_and_refused_under_strict() {
    let plugins = r#"{"name":"pkg","category":"development","source":{"source":"npm","package":"@acme/pkg"}}"#;

    let (_dest, report) =
        import(&kit(plugins), &Unreachable, false).expect("lenient import succeeds");
    assert!(report.plugins.is_empty());
    assert!(
        report
            .warnings
            .contains(&ImportWarning::RemotePluginSource {
                plugin: "pkg".to_owned()
            })
    );

    assert!(import(&kit(plugins), &Unreachable, true).is_err());
}

#[test]
fn a_remote_url_that_is_not_public_https_is_refused() {
    let plugins = r#"{"name":"x","category":"development","source":{"source":"url","url":"git@github.com:acme/x.git","sha":"efc7d4633dfb8fd05baeb8d96fa17f4bb26de498"}}"#;

    let err = import(&kit(plugins), &Unreachable, false).expect_err("ssh url refused");

    assert!(
        err.to_string()
            .contains("neither `owner/repository` nor an https URL"),
        "{err}"
    );
}

fn marketplace_yaml(dest: &Path) -> serde_yaml::Value {
    let text = std::fs::read_to_string(dest.join("marketplaces/acme/config.yaml"))
        .expect("marketplace config written");
    serde_yaml::from_str(&text).expect("valid yaml")
}

const PASS_THROUGH: &str = r#"{"name":"playwright-cli","mode":"pass_through","source":{"source":"git-subdir","url":"microsoft/playwright-cli","path":"skills","ref":"v0.1.21","sha":"74354ecc7a43da16d91a9bc54fa8db8283a3fcf5"},"strict":false,"skills":["./"],"version":"0.1.21"}"#;

#[test]
fn a_pass_through_entry_is_kept_as_authored_and_never_fetched() {
    let tree = kit(PASS_THROUGH);

    let (dest, report) = import(&tree, &Unreachable, true).expect("strict import succeeds");

    assert!(report.plugins.is_empty(), "nothing is vendored");
    assert!(report.upstream.is_empty(), "nothing is fetched");
    assert!(!dest.path().join("plugins/playwright-cli").exists());
    let marketplace = &marketplace_yaml(dest.path())["marketplace"];
    assert_eq!(
        marketplace["plugins"]["include"],
        serde_yaml::Value::Sequence(vec![])
    );
    let expected: serde_yaml::Value = serde_yaml::from_str(
        r#"
- name: playwright-cli
  source:
    source: git-subdir
    url: microsoft/playwright-cli
    path: skills
    ref: v0.1.21
    sha: 74354ecc7a43da16d91a9bc54fa8db8283a3fcf5
  version: 0.1.21
  strict: false
  skills: ["./"]
"#,
    )
    .expect("expected yaml");
    assert_eq!(marketplace["external_plugins"], expected);
}

#[test]
fn a_pass_through_entry_without_a_sha_is_refused_naming_it() {
    let tree = kit(
        r#"{"name":"playwright-cli","mode":"pass_through","source":{"source":"git-subdir","url":"microsoft/playwright-cli","path":"skills","ref":"v0.1.21"}}"#,
    );

    let err = import(&tree, &Unreachable, false).expect_err("unpinned refused");

    let text = err.to_string();
    assert!(
        text.contains("pass-through plugin 'playwright-cli'"),
        "{text}"
    );
    assert!(text.contains("sha"), "{text}");
}

#[test]
fn a_pass_through_entry_carrying_unmodelled_keys_is_refused() {
    let tree = kit(&PASS_THROUGH.replacen('{', r#"{"category":"testing","#, 1));

    let err = import(&tree, &Unreachable, false).expect_err("category refused");

    assert!(err.to_string().contains("remove category"), "{err}");
}

#[test]
fn a_pass_through_entry_beside_a_vendored_one_leaves_the_vendored_one_imported() {
    let tree = kit(&format!("{},{PASS_THROUGH}", git_subdir("b2c", "")));
    let capture = FakeCapture::new(vec![("skills/one/SKILL.md", skill("One."))]);

    let (dest, report) = import(&tree, &capture, true).expect("import succeeds");

    assert_eq!(
        capture.seen().len(),
        1,
        "only the vendored entry is fetched"
    );
    assert_eq!(report.plugins.len(), 1);
    let marketplace = &marketplace_yaml(dest.path())["marketplace"];
    assert_eq!(marketplace["plugins"]["include"][0], "b2c");
    assert_eq!(marketplace["external_plugins"][0]["name"], "playwright-cli");
}
