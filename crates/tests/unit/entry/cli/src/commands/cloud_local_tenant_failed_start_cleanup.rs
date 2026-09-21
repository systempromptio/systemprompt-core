#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

#[cfg(unix)]
#[tokio::test]
async fn failed_local_database_start_removes_its_owned_compose_project() {
    use std::os::unix::fs::PermissionsExt;

    use systemprompt_cli::ScriptedPrompter;
    use systemprompt_cli::cloud::tenant::create::{create_local_tenant, sanitize_database_name};

    let root = tempfile::TempDir::new().expect("owned project directory");
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let calls = root.path().join("docker.calls");
    let docker = bin.join("docker");
    std::fs::write(
        &docker,
        format!(
            "#!/bin/sh\nlog='{}'\nprintf 'CALL=%s\\n' \"$*\" >> \"$log\"\nprevious=''\ncompose_file=''\nfor argument in \"$@\"; do\n  if [ \"$previous\" = '-f' ]; then compose_file=\"$argument\"; fi\n  previous=\"$argument\"\ndone\ncase \" $* \" in\n  *' up -d '*)\n    test -f \"$compose_file\" || exit 44\n    printf 'UP_FILE=%s\\n' \"$compose_file\" >> \"$log\"\n    exit 19\n    ;;\n  *' down -v '*)\n    test -f \"$compose_file\" || exit 45\n    printf 'DOWN_FILE=%s\\n' \"$compose_file\" >> \"$log\"\n    exit 0\n    ;;\n  *) exit 0 ;;\nesac\n",
            calls.display(),
        ),
    )
    .unwrap();
    std::fs::set_permissions(&docker, std::fs::Permissions::from_mode(0o755)).unwrap();

    let old_cwd = std::env::current_dir().unwrap();
    let old_path = std::env::var_os("PATH");
    std::env::set_current_dir(root.path()).unwrap();
    unsafe {
        std::env::set_var(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                old_path.as_deref().unwrap_or_default().to_string_lossy()
            ),
        );
    }

    let prompter = ScriptedPrompter::new(["Coverage Tenant", "55432"]);
    let error = create_local_tenant(&prompter)
        .await
        .expect_err("owned docker start failure must reach cleanup");

    std::env::set_current_dir(old_cwd).unwrap();
    unsafe {
        if let Some(path) = old_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
    }

    assert!(
        format!("{error:#}").contains("Failed to start PostgreSQL container"),
        "unexpected failure: {error:#}"
    );
    let invocations = std::fs::read_to_string(&calls).expect("docker invocation log");
    let lines: Vec<&str> = invocations
        .lines()
        .filter_map(|line| line.strip_prefix("CALL="))
        .collect();
    assert_eq!(lines.len(), 3, "unexpected Docker calls: {invocations}");
    let project_prefix = format!(
        "ps -q -f label=com.docker.compose.project={}_",
        sanitize_database_name("Coverage Tenant")
    );
    assert!(
        lines[0].starts_with(&project_prefix),
        "unexpected Docker calls: {invocations}"
    );
    let project = lines[0]
        .strip_prefix("ps -q -f label=com.docker.compose.project=")
        .unwrap();
    assert!(lines[1].contains(&format!("compose -p {project} ")));
    assert!(lines[1].ends_with(" up -d"));
    assert!(lines[2].contains(&format!("compose -p {project} ")));
    assert!(lines[2].ends_with(" down -v"));
    let up_path = invocations
        .lines()
        .find_map(|line| line.strip_prefix("UP_FILE="))
        .expect("up command observed an existing compose file");
    let down_path = invocations
        .lines()
        .find_map(|line| line.strip_prefix("DOWN_FILE="))
        .expect("cleanup observed the same compose file before removing it");
    assert_eq!(up_path, down_path);
    let compose_path = std::path::Path::new(up_path);
    assert!(
        compose_path.starts_with(root.path().join(".systemprompt/docker")),
        "compose path escaped the owned project: {up_path}"
    );
    let expected_file = format!("{project}.yaml");
    assert_eq!(
        compose_path.file_name().and_then(std::ffi::OsStr::to_str),
        Some(expected_file.as_str())
    );
    assert!(
        !compose_path.exists(),
        "failed provisioning must remove its compose file"
    );
}
