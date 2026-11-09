use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use systemprompt_bridge::update::swap::{SwapNonce, leftovers, swap, sweep};
use tempfile::TempDir;

const HELPER: &str = "update_swap_windows::running_image_helper";

struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _killed = self.0.kill();
        let _reaped = self.0.wait();
    }
}

impl Running {
    fn stop(mut self) {
        let _killed = self.0.kill();
        self.0.wait().expect("reap the running image");
    }
}

fn launch(image: &Path) -> Running {
    let child = Command::new(image)
        .args(["--ignored", "--exact", HELPER, "--nocapture"])
        .spawn()
        .expect("start the installed image");
    let running = Running(child);
    std::thread::sleep(Duration::from_millis(500));
    running
}

fn remove_eventually(target: &Path, leftover: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while leftover.exists() && Instant::now() < deadline {
        sweep(target);
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
#[ignore = "spawned by the running-image swap test as the image it keeps mapped"]
fn running_image_helper() {
    std::thread::sleep(Duration::from_secs(60));
}

#[test]
fn a_second_swap_succeeds_while_the_first_displaced_image_is_still_running() {
    let dir = TempDir::new().expect("dir");
    let target: PathBuf = dir.path().join("bridge.exe");
    let image = std::env::current_exe().expect("test image");
    std::fs::copy(&image, &target).expect("install the image");
    let staged = dir.path().join("download.exe");
    std::fs::copy(&image, &staged).expect("stage the update");

    let running = launch(&target);
    let first = swap(&staged, &target, &SwapNonce::generate()).expect("swap over a running image");
    assert!(first.exists(), "the running image moved aside");

    let second = swap(&staged, &target, &SwapNonce::generate())
        .expect("swap again while the first displaced image still runs");
    assert!(second.exists());

    let kept = sweep(&target);
    assert!(
        kept.contains(&first),
        "the running displaced image is left in place: {kept:?}"
    );
    assert!(!second.exists(), "an idle displaced image is removed");

    running.stop();
    remove_eventually(&target, &first);
    assert!(
        leftovers(&target).is_empty(),
        "every leftover is removed once nothing runs from it: {:?}",
        leftovers(&target)
    );
    assert!(target.exists());
}
