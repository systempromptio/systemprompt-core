use std::cell::Cell;
use std::path::Path;

use systemprompt_bridge::update::UpdateError;
use systemprompt_bridge::update::swap::{
    SwapNonce, displaced_path, incoming_path, leftovers, swap, swap_with, sweep,
};
use tempfile::TempDir;

struct Install {
    _dir: TempDir,
    staged: std::path::PathBuf,
    target: std::path::PathBuf,
}

fn install() -> Install {
    let dir = TempDir::new().expect("dir");
    let staged = dir.path().join("download");
    let target = dir.path().join("bridge.exe");
    std::fs::write(&staged, b"new").expect("staged");
    std::fs::write(&target, b"old").expect("target");
    Install {
        _dir: dir,
        staged,
        target,
    }
}

fn nonce(seed: char) -> SwapNonce {
    SwapNonce::parse(&seed.to_string().repeat(32)).expect("a 32-hex nonce")
}

fn contents(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("read")
}

#[test]
fn a_fresh_swap_puts_the_new_binary_at_the_target_and_keeps_the_old_aside() {
    let i = install();
    let displaced = swap(&i.staged, &i.target, &nonce('1')).expect("swap");
    assert_eq!(contents(&i.target), b"new");
    assert_eq!(contents(&displaced), b"old");
    assert_eq!(displaced, displaced_path(&i.target, &nonce('1')));
    assert!(!incoming_path(&i.target, &nonce('1')).exists());
}

#[test]
fn a_swap_succeeds_while_earlier_displaced_binaries_remain() {
    let i = install();
    let legacy = i.target.with_file_name("bridge.exe.old");
    std::fs::write(&legacy, b"legacy").expect("legacy leftover");
    let first = swap(&i.staged, &i.target, &nonce('1')).expect("first swap");
    std::fs::write(&i.staged, b"newer").expect("second staged");
    let second = swap(&i.staged, &i.target, &nonce('2')).expect("second swap");
    assert_eq!(contents(&i.target), b"newer");
    assert_eq!(contents(&first), b"old");
    assert_eq!(contents(&second), b"new");
    assert_eq!(
        contents(&legacy),
        b"legacy",
        "a leftover is never touched by a swap"
    );
}

#[test]
fn every_swap_displaces_to_a_name_of_its_own() {
    let target = Path::new("/opt/bridge/bridge.exe");
    let a = SwapNonce::generate();
    let b = SwapNonce::generate();
    assert_ne!(a, b);
    assert_ne!(displaced_path(target, &a), displaced_path(target, &b));
    assert_eq!(displaced_path(target, &a).parent(), target.parent());
    assert!(
        displaced_path(target, &a)
            .to_string_lossy()
            .ends_with(".old")
    );
}

#[test]
fn a_failed_final_rename_restores_the_previous_binary() {
    let i = install();
    let calls = Cell::new(0);
    let rename = |from: &Path, to: &Path| {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        }
        std::fs::rename(from, to)
    };
    let err =
        swap_with(&i.staged, &i.target, &nonce('1'), &rename).expect_err("final rename fails");
    assert!(matches!(err, UpdateError::ReplaceBlocked { .. }), "{err:?}");
    assert_eq!(contents(&i.target), b"old", "the previous binary is back");
    assert!(!displaced_path(&i.target, &nonce('1')).exists());
    assert!(!incoming_path(&i.target, &nonce('1')).exists());
}

#[test]
fn a_blocked_first_rename_leaves_the_install_untouched() {
    let i = install();
    let rename =
        |_: &Path, _: &Path| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    let err = swap_with(&i.staged, &i.target, &nonce('1'), &rename).expect_err("blocked");
    assert!(matches!(err, UpdateError::ReplaceBlocked { .. }), "{err:?}");
    assert!(
        err.user_message().contains("run the installer again"),
        "{}",
        err.user_message()
    );
    assert_eq!(contents(&i.target), b"old");
    assert!(!incoming_path(&i.target, &nonce('1')).exists());
}

#[test]
fn sweeping_removes_every_displaced_binary_including_the_fixed_legacy_name() {
    let i = install();
    let legacy = i.target.with_file_name("bridge.exe.old");
    std::fs::write(&legacy, b"legacy").expect("legacy");
    let displaced = swap(&i.staged, &i.target, &nonce('1')).expect("swap");
    let unrelated = i.target.with_file_name("other.exe.old");
    std::fs::write(&unrelated, b"x").expect("unrelated");

    let mut found = leftovers(&i.target);
    found.sort();
    let mut expected = vec![legacy.clone(), displaced.clone()];
    expected.sort();
    assert_eq!(found, expected);

    assert!(sweep(&i.target).is_empty(), "nothing is in use here");
    assert!(!legacy.exists());
    assert!(!displaced.exists());
    assert!(unrelated.exists(), "another program's leftover is not ours");
    assert_eq!(contents(&i.target), b"new");
}

#[test]
fn a_generated_nonce_has_the_shape_the_sweep_recognises() {
    let generated = SwapNonce::generate();
    assert_eq!(SwapNonce::parse(generated.as_str()), Some(generated));
    assert_eq!(SwapNonce::parse("n1"), None);
    assert_eq!(SwapNonce::parse(&"A".repeat(32)), None);
    assert_eq!(SwapNonce::parse(&"a".repeat(31)), None);
}

#[test]
fn sweeping_removes_an_interrupted_incoming_copy() {
    let i = install();
    let stranded = incoming_path(&i.target, &nonce('3'));
    std::fs::write(&stranded, b"half copied").expect("stranded incoming");
    assert_eq!(leftovers(&i.target), vec![stranded.clone()]);
    assert!(sweep(&i.target).is_empty());
    assert!(!stranded.exists());
}

#[test]
fn sweeping_never_touches_a_user_file_that_only_looks_like_a_leftover() {
    let i = install();
    let user_files = [
        "bridge.exe.backup.old",
        "bridge.exe.2024.old",
        "bridge.exe.new",
        "bridge.exe.OLD",
        "bridge.exe.0123456789abcdef0123456789ABCDEF.old",
        "bridge.exe.0123456789abcdef0123456789abcde.old",
        "bridge.exe.0123456789abcdef0123456789abcdef.old.txt",
        "bridge.exeX.old",
    ];
    for name in user_files {
        std::fs::write(i.target.with_file_name(name), b"keep").expect("user file");
    }
    assert!(
        leftovers(&i.target).is_empty(),
        "{:?}",
        leftovers(&i.target)
    );
    sweep(&i.target);
    for name in user_files {
        assert!(i.target.with_file_name(name).exists(), "{name} was deleted");
    }
}
