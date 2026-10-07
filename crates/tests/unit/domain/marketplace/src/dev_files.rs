//! Dev-only files stay out of the imported tree, the captured revision and
//! the consumer plan; a kit's `.systempromptignore` adds to the defaults.

use std::fs;
use std::path::Path;

use systemprompt_identifiers::SkillId;
use systemprompt_marketplace::managed::capture_skills;
use systemprompt_marketplace::{DevFileFilter, ImportOptions, import_anthropic_tree};
use tempfile::TempDir;

use crate::import_tree::fixture;

const SKILL: &str = "plugins/alpha-tools/skills/alpha_discovery";

fn copy_dir(src: &Path, dest: &Path) {
    fs::create_dir_all(dest).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let path = entry.unwrap().path();
        let target = dest.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

fn write(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn kit_with_dev_files(ignore: Option<&str>) -> TempDir {
    let kit = TempDir::new().unwrap();
    copy_dir(&fixture("anthropic"), kit.path());
    for rel in [
        "README.md",
        "tests/discovery.test.ts",
        "foo.test.ts",
        "scripts/plan.spec.py",
        "fixtures/sample.json",
        "scripts/fixtures/input.txt",
        "notes/draft.md",
        "scripts/run.snap",
        "scripts/helper.sh",
        "references/README.md",
    ] {
        write(kit.path(), &format!("{SKILL}/{rel}"), "dev\n");
    }
    if let Some(text) = ignore {
        write(kit.path(), ".systempromptignore", text);
    }
    kit
}

fn import(kit: &TempDir) -> TempDir {
    let dest = TempDir::new().unwrap();
    import_anthropic_tree(
        kit.path(),
        dest.path(),
        &ImportOptions::new(std::env::temp_dir()),
    )
    .unwrap();
    dest
}

#[test]
fn default_excludes_drop_readme_tests_fixtures_and_specs() {
    let filter = DevFileFilter::defaults();
    for path in [
        "README.md",
        "readme.MD",
        "tests/a.sh",
        "test/a.sh",
        "scripts/__tests__/a.js",
        "fixtures/a.json",
        "foo.test.ts",
        "scripts/plan.spec.py",
    ] {
        assert!(
            filter.excludes(path, None, false),
            "{path} must be excluded"
        );
    }
    for path in [
        "SKILL.md",
        "references/README.md",
        "scripts/test.py",
        "scripts/testing/a.sh",
        "contest.md",
    ] {
        assert!(!filter.excludes(path, None, false), "{path} must ship");
    }
}

#[test]
fn imported_skill_ships_without_dev_files() {
    let kit = kit_with_dev_files(None);
    let dest = import(&kit);
    let skill = dest.path().join("skills/alpha_discovery");
    assert!(skill.join("SKILL.md").is_file());
    assert!(skill.join("checklist.md").is_file());
    assert!(skill.join("scripts/helper.sh").is_file());
    assert!(skill.join("references/README.md").is_file());
    assert!(skill.join("notes/draft.md").is_file());
    for gone in [
        "README.md",
        "tests",
        "foo.test.ts",
        "scripts/plan.spec.py",
        "fixtures",
        "scripts/fixtures",
    ] {
        assert!(!skill.join(gone).exists(), "{gone} must not be imported");
    }
}

#[test]
fn kit_ignore_file_adds_patterns_and_can_reinclude_a_default() {
    let kit = kit_with_dev_files(Some(
        "# kit dev paths\n*.snap\n/plugins/alpha-tools/skills/alpha_discovery/notes/\n!fixtures/\n",
    ));
    let dest = import(&kit);
    let skill = dest.path().join("skills/alpha_discovery");
    assert!(!skill.join("scripts/run.snap").exists());
    assert!(!skill.join("notes").exists());
    assert!(skill.join("fixtures/sample.json").is_file());
    assert!(skill.join("scripts/helper.sh").is_file());
    assert!(!skill.join("README.md").exists());
    assert!(!skill.join("tests").exists());
}

#[test]
fn invalid_ignore_pattern_refuses_the_import() {
    let kit = kit_with_dev_files(Some("[unclosed\n"));
    let dest = TempDir::new().unwrap();
    let err = import_anthropic_tree(
        kit.path(),
        dest.path(),
        &ImportOptions::new(std::env::temp_dir()),
    )
    .expect_err("a malformed ignore file is an error");
    assert!(err.to_string().contains(".systempromptignore"));
}

#[test]
fn managed_capture_skips_dev_files_and_honours_the_ignore_file() {
    let root = TempDir::new().unwrap();
    write(
        root.path(),
        "skills/alpha/config.yaml",
        "id: alpha\nname: Alpha\ndescription: Capture fixture\nenabled: true\nfile: index.md\n",
    );
    for rel in [
        "index.md",
        "README.md",
        "tests/a.sh",
        "foo.test.ts",
        "scripts/run.sh",
        "scripts/out.snap",
    ] {
        write(root.path(), &format!("skills/alpha/{rel}"), "x\n");
    }
    write(root.path(), ".systempromptignore", "*.snap\n");
    let captured = capture_skills(root.path(), &[SkillId::new("alpha")]).unwrap();
    let mut paths: Vec<&str> = captured.skills()["alpha"]
        .0
        .keys()
        .map(String::as_str)
        .collect();
    paths.sort_unstable();
    assert_eq!(paths, vec!["config.yaml", "index.md", "scripts/run.sh"]);
}
