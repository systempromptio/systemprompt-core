use std::path::Path;

use harness_config::ConfigError;

use crate::support::{load, paths, repo, write};

fn skill(root: &Path, dir: &str, text: &str) {
    write(&root.join(dir).join("SKILL.md"), text);
}

#[test]
fn discovery_rules() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    let far = paths.config.join("skills");
    let near = repo.join(".harness").join("skills");
    skill(
        &far,
        "pdf",
        "---\nname: pdf\ndescription: far pdf\n---\nFar body",
    );
    skill(
        &far,
        "only-far",
        "---\nname: only-far\ndescription: x\nlicense: MIT\n---\n",
    );
    skill(
        &near,
        "pdf",
        "---\nname: pdf\ndescription: near pdf\n---\nNear body",
    );
    skill(
        &near,
        ".secret",
        "---\nname: .secret\ndescription: hidden\n---\n",
    );
    skill(&near, "wrong", "---\nname: other\ndescription: x\n---\n");
    skill(&near, "nodesc", "---\nname: nodesc\n---\n");
    std::fs::create_dir_all(near.join("empty")).unwrap();
    write(&near.join("loose.md"), "not a skill");

    let loaded = load(&paths, &repo);
    let skills = &loaded.skills;
    assert_eq!(skills.keys().collect::<Vec<_>>(), ["only-far", "pdf"]);
    assert_eq!(skills["pdf"].description, "near pdf");
    assert_eq!(skills["pdf"].body, "Near body");
    assert_eq!(skills["pdf"].dir, near.join("pdf"));
    assert_eq!(skills["only-far"].extra["license"], "MIT");
    let problems = &loaded.problems;
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(
        problems
            .iter()
            .any(|p| matches!(p, ConfigError::SkillName { name, .. } if name == "other"))
    );
}
