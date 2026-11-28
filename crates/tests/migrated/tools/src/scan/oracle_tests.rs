//! The scanner against a real shell: every command bash actually runs for a
//! line is one the scanner reported.

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;

use super::posix;

const STUBS: [&str; 6] = ["alpha", "beta", "gamma", "delta", "omega", "sigma"];

fn stub_dir(root: &Path, log: &Path) {
    for name in STUBS {
        let path = root.join(name);
        let script = format!("#!/bin/sh\necho {name} >> '{}'\n", log.display());
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn executed(line: &str) -> BTreeSet<String> {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = dir.path().join("ran.log");
    stub_dir(&bin, &log);
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let status = Command::new("/bin/sh")
        .arg("-c")
        .arg(line)
        .env("PATH", path)
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.code().is_some(), "{line}");
    std::fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn every_command_the_shell_runs_is_reported() {
    let lines = [
        "alpha; beta && gamma || delta",
        "alpha | beta",
        "echo $(alpha) `beta`",
        "X=$(gamma) delta",
        "if alpha; then beta; fi",
        "f() { gamma; }; f",
        "{ alpha; } > /dev/null; beta",
        "for i in 1 2; do omega; done",
        "case x in x) sigma ;; esac",
        "echo ${UNSET:-$(alpha)}",
        "cat <<EOF\n$(beta)\nEOF",
        "( alpha ) && { beta; }",
        "while false; do alpha; done; gamma",
        "true && delta",
    ];
    for line in lines {
        let ran = executed(line);
        let reported: BTreeSet<String> = posix(line).into_iter().collect();
        let missed: Vec<&String> = ran.difference(&reported).collect();
        assert!(
            missed.is_empty(),
            "{line:?}: ran {ran:?}, reported {reported:?}"
        );
    }
}
