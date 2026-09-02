use std::sync::atomic::{AtomicUsize, Ordering};

use systemprompt_bridge::schedule::status::{ScheduleStatus, ScheduleStatusCache};
use systemprompt_bridge::verdict::Tone;

#[test]
fn each_status_carries_the_tone_the_gui_renders() {
    assert_eq!(ScheduleStatus::Installed.tone(), Tone::Ok);
    assert_eq!(ScheduleStatus::NotInstalled.tone(), Tone::Warn);
    assert_eq!(ScheduleStatus::Unknown.tone(), Tone::Unknown);
}

#[test]
fn the_verdict_pairs_the_tone_with_the_status_itself() {
    let verdict = ScheduleStatus::NotInstalled.verdict();
    assert_eq!(verdict.tone, Tone::Warn);
    assert_eq!(verdict.code, ScheduleStatus::NotInstalled);
}

#[test]
fn the_wire_code_is_kebab_case() {
    let json = serde_json::to_string(&ScheduleStatus::NotInstalled).expect("status serialises");
    assert_eq!(json, "\"not-installed\"");
    let verdict =
        serde_json::to_value(ScheduleStatus::Installed.verdict()).expect("verdict serialises");
    assert_eq!(verdict["tone"], "ok");
    assert_eq!(verdict["code"], "installed");
}

fn counting_probe(count: &AtomicUsize, status: ScheduleStatus) -> impl FnOnce() -> ScheduleStatus {
    move || {
        count.fetch_add(1, Ordering::SeqCst);
        status
    }
}

#[test]
fn a_conclusive_schedule_answer_is_probed_once_and_then_cached() {
    let cache = ScheduleStatusCache::default();
    let calls = AtomicUsize::new(0);

    let first = cache.schedule(counting_probe(&calls, ScheduleStatus::Installed));
    let second = cache.schedule(counting_probe(&calls, ScheduleStatus::NotInstalled));

    assert_eq!(first, ScheduleStatus::Installed);
    assert_eq!(
        second,
        ScheduleStatus::Installed,
        "the cached answer is returned, so the second probe's value is never seen"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the scheduler subprocess is spawned once, not on every tray redraw"
    );
}

#[test]
fn an_unknown_answer_is_never_cached() {
    let cache = ScheduleStatusCache::default();
    let calls = AtomicUsize::new(0);

    let first = cache.schedule(counting_probe(&calls, ScheduleStatus::Unknown));
    let second = cache.schedule(counting_probe(&calls, ScheduleStatus::Installed));
    let third = cache.schedule(counting_probe(&calls, ScheduleStatus::NotInstalled));

    assert_eq!(first, ScheduleStatus::Unknown);
    assert_eq!(
        second,
        ScheduleStatus::Installed,
        "a transient failure must not become the permanent answer"
    );
    assert_eq!(
        third,
        ScheduleStatus::Installed,
        "the conclusive answer sticks"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the probe reruns after Unknown and stops once conclusive"
    );
}

#[test]
fn autostart_and_schedule_are_cached_independently() {
    let cache = ScheduleStatusCache::default();
    let calls = AtomicUsize::new(0);

    let schedule = cache.schedule(counting_probe(&calls, ScheduleStatus::Installed));
    let autostart = cache.autostart(counting_probe(&calls, ScheduleStatus::NotInstalled));

    assert_eq!(schedule, ScheduleStatus::Installed);
    assert_eq!(
        autostart,
        ScheduleStatus::NotInstalled,
        "the autostart registration has its own cell"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn writing_through_the_cache_replaces_the_stored_answer_without_probing() {
    let cache = ScheduleStatusCache::default();
    let calls = AtomicUsize::new(0);

    assert_eq!(
        cache.schedule(counting_probe(&calls, ScheduleStatus::NotInstalled)),
        ScheduleStatus::NotInstalled
    );
    cache.set_schedule(ScheduleStatus::Installed);
    cache.set_autostart(ScheduleStatus::Installed);

    assert_eq!(
        cache.schedule(counting_probe(&calls, ScheduleStatus::Unknown)),
        ScheduleStatus::Installed,
        "install writes the new registration state through the cache"
    );
    assert_eq!(
        cache.autostart(counting_probe(&calls, ScheduleStatus::Unknown)),
        ScheduleStatus::Installed,
        "a write-through seeds a cell that was never probed"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "no probe runs once the answer has been written through"
    );
}
