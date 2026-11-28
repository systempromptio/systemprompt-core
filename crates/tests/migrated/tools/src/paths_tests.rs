//! Where a tool path lands and when it needs outside-directory approval.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;
use systemprompt_engine::tools::{
    ReadLimits, ReadTool, display_path, guard_external, is_within, normalize, resolve,
};

use crate::support::{Gate, Project, context, slash};

async fn asks_outside(project: &Project, cwd: &Path, path: &Path) -> Vec<String> {
    let gate = Arc::new(Gate::default());
    let mut ctx = context(&project.root, Arc::clone(&gate));
    ctx.cwd = cwd.to_path_buf();
    guard_external(&ctx, path, "Read").await.unwrap();
    gate.asked()
        .into_iter()
        .flat_map(|request| request.always)
        .collect()
}

#[tokio::test]
async fn a_relative_existing_file_resolves_inside_the_project() {
    let project = Project::new();
    project.put("src/lib.rs", b"x");
    let path = resolve(&project.root, "src/lib.rs");
    assert_eq!(path, project.root.join("src").join("lib.rs"));
    assert!(is_within(&path, &project.root));
    assert!(
        asks_outside(&project, &project.root, &path)
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn a_relative_file_that_does_not_exist_yet_resolves_inside() {
    let project = Project::new();
    let path = resolve(&project.root, "new/dir/file.rs");
    assert_eq!(path, project.root.join("new").join("dir").join("file.rs"));
    assert!(is_within(&path, &project.root));
    assert!(
        asks_outside(&project, &project.root, &path)
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn a_relative_path_that_climbs_out_asks_for_the_outside_directory() {
    let project = Project::new();
    let outside = project.outside();
    let path = resolve(&project.root, "../elsewhere/notes.txt");
    assert_eq!(path, outside.join("notes.txt"));
    let always = asks_outside(&project, &project.root, &path).await;
    assert_eq!(always, [format!("{}/*", slash(&outside))]);
}

#[tokio::test]
async fn a_path_outside_the_working_directory_but_inside_the_project_asks_nothing() {
    let project = Project::new();
    project.put("pkg/a/file.txt", b"x");
    let cwd = project.root.join("pkg").join("b");
    std::fs::create_dir_all(&cwd).unwrap();
    let path = resolve(&cwd, "../a/file.txt");
    assert_eq!(path, project.root.join("pkg").join("a").join("file.txt"));
    assert!(asks_outside(&project, &cwd, &path).await.is_empty());
}

#[tokio::test]
#[ignore = "parity ledger: a project rooted at the filesystem root makes every path internal"]
async fn a_project_rooted_at_the_filesystem_root_is_not_a_boundary_that_contains_everything() {
    let project = Project::new();
    let outside = project.outside();
    let gate = Arc::new(Gate::default());
    let mut ctx = context(&project.root, Arc::clone(&gate));
    ctx.worktree = PathBuf::from("/");
    guard_external(&ctx, &outside.join("f.txt"), "Read")
        .await
        .unwrap();
    assert_eq!(gate.permissions(), ["external_directory"]);
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "parity ledger: a path below a symlink that leads outside is judged by its target"]
async fn a_new_file_below_a_symlink_to_outside_is_judged_by_its_lexical_place() {
    let project = Project::new();
    let outside = project.outside();
    std::os::unix::fs::symlink(&outside, project.root.join("linked")).unwrap();
    let path = resolve(&project.root, "linked/new.txt");
    assert!(
        asks_outside(&project, &project.root, &path)
            .await
            .is_empty()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlink_inside_the_project_to_another_place_inside_is_followed() {
    let project = Project::new();
    project.put("real/file.txt", b"x");
    std::os::unix::fs::symlink(project.root.join("real"), project.root.join("alias")).unwrap();
    let path = resolve(&project.root, "alias/file.txt");
    assert!(is_within(&path, &project.root));
    assert_eq!(display_path(&path, &project.root), "real/file.txt");
    assert!(
        asks_outside(&project, &project.root, &path)
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn an_absolute_path_inside_the_project_needs_no_outside_approval() {
    let project = Project::new();
    let absolute = project.put("a.txt", b"x");
    let path = resolve(&project.root, &absolute.to_string_lossy());
    assert_eq!(path, absolute);
    assert!(
        asks_outside(&project, &project.root, &path)
            .await
            .is_empty()
    );
}

#[tokio::test]
async fn an_absolute_path_outside_the_project_asks_for_its_directory() {
    let project = Project::new();
    let outside = project.outside();
    let path = resolve(&project.root, &outside.join("deep").to_string_lossy());
    std::fs::create_dir_all(&path).unwrap();
    let always = asks_outside(&project, &project.root, &path).await;
    assert_eq!(always, [format!("{}/*", slash(&path))]);
}

#[tokio::test]
async fn an_existing_file_outside_is_approved_at_its_directory() {
    let project = Project::new();
    let outside = project.outside();
    let file = crate::support::put(&outside, "notes.txt", b"x");
    let always = asks_outside(&project, &project.root, &file).await;
    assert_eq!(always, [format!("{}/*", slash(&outside))]);
}

#[tokio::test]
async fn a_new_file_deep_outside_is_approved_at_its_lexical_parent() {
    let project = Project::new();
    let outside = project.outside();
    let file = outside.join("new").join("deeper").join("file.txt");
    let always = asks_outside(&project, &project.root, &file).await;
    assert_eq!(
        always,
        [format!("{}/*", slash(&outside.join("new").join("deeper")))]
    );
}

#[tokio::test]
#[ignore = "parity ledger: outside approval is saved at the file's directory, not at its repository"]
async fn outside_approval_is_saved_at_the_enclosing_repository() {
    let project = Project::new();
    let outside = project.outside();
    std::fs::create_dir_all(outside.join("repo").join(".git")).unwrap();
    let file = crate::support::put(&outside, "repo/src/a.txt", b"x");
    let always = asks_outside(&project, &project.root, &file).await;
    assert_eq!(always, [format!("{}/*", slash(&outside.join("repo")))]);
}

#[tokio::test]
#[ignore = "parity ledger: a leading tilde is not expanded in tool paths"]
async fn a_leading_tilde_expands_to_the_home_directory() {
    let project = Project::new();
    let path = resolve(&project.root, "~/notes.txt");
    assert!(!path.starts_with(&project.root));
}

#[tokio::test]
async fn a_read_of_a_tilde_path_never_reads_outside_without_asking() {
    let project = Project::new();
    project.put("~/notes.txt", b"inside");
    let called = project
        .call(
            &ReadTool::new(ReadLimits::default()),
            json!({"path": "~/notes.txt"}),
        )
        .await;
    let asked = called.gate.permissions();
    let out = called.ok();
    assert_eq!(asked, ["read"]);
    assert!(out.output.contains("inside"));
}

#[test]
fn dot_segments_collapse_lexically() {
    assert_eq!(
        normalize(Path::new("a/./b/../c/./d")),
        PathBuf::from("a").join("c").join("d")
    );
    assert_eq!(
        normalize(Path::new("../a/../../b")),
        PathBuf::from("../../b")
    );
    assert_eq!(normalize(Path::new("/../a")), PathBuf::from("/a"));
}

#[test]
fn joining_an_absolute_input_ignores_the_working_directory() {
    let cwd = std::env::temp_dir();
    let absolute = cwd.join("x").join("..").join("y");
    assert_eq!(
        resolve(Path::new("/elsewhere"), &absolute.to_string_lossy()),
        cwd.join("y")
    );
    assert_eq!(resolve(&cwd, "  spaced.txt  "), cwd.join("spaced.txt"));
}

#[test]
fn relating_a_path_to_the_project_gives_a_slash_separated_relative_name() {
    let project = Project::new();
    let path = project.root.join("src").join("deep").join("a.rs");
    assert_eq!(display_path(&path, &project.root), "src/deep/a.rs");
    let outside = project.outside().join("b.rs");
    assert_eq!(display_path(&outside, &project.root), slash(&outside));
}

#[cfg(unix)]
#[test]
fn backslashes_are_ordinary_characters_on_posix() {
    assert_eq!(normalize(Path::new("a\\b/../c\\d")), PathBuf::from("c\\d"));
    let cwd = Path::new("/work");
    assert_eq!(resolve(cwd, "\\x"), PathBuf::from("/work/\\x"));
}

#[cfg(windows)]
#[test]
fn windows_paths_resolve_with_either_separator_and_a_drive() {
    let cwd = Path::new(r"C:\work\proj");
    assert_eq!(
        resolve(cwd, r"src\..\a.txt"),
        PathBuf::from(r"C:\work\proj\a.txt")
    );
    assert_eq!(
        resolve(cwd, "src/b.txt"),
        PathBuf::from(r"C:\work\proj\src\b.txt")
    );
    assert_eq!(
        resolve(cwd, r"D:\other\.\c.txt"),
        PathBuf::from(r"D:\other\c.txt")
    );
}

#[cfg(windows)]
#[test]
fn verbatim_prefixes_are_removed() {
    assert_eq!(
        normalize(Path::new(r"\\?\C:\work\a.txt")),
        PathBuf::from(r"C:\work\a.txt")
    );
}

#[cfg(windows)]
#[test]
#[ignore = "parity ledger: a rooted path without a drive is not given the working directory's drive"]
fn a_rooted_path_without_a_drive_takes_the_working_directorys_drive() {
    assert_eq!(
        resolve(Path::new(r"D:\work"), r"\tmp\x"),
        PathBuf::from(r"D:\tmp\x")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_unreadable_link_is_an_unresolvable_path_not_a_failure() {
    let project = Project::new();
    std::os::unix::fs::symlink(project.root.join("absent"), project.root.join("dangling")).unwrap();
    let path = resolve(&project.root, "dangling");
    assert!(is_within(&path, &project.root));
    assert_eq!(display_path(&path, &project.root), "dangling");
}
