//! The memo that keeps the Start-menu probe from cold-starting PowerShell on
//! every 30-second host probe.

use systemprompt_bridge::probe_cache::{StartMenuCache, StartMenuPresence};

#[test]
fn a_probe_answer_maps_onto_the_presence_it_recorded_and_back_again() {
    for (probe, presence) in [
        (Some(true), StartMenuPresence::Present),
        (Some(false), StartMenuPresence::Absent),
        (None, StartMenuPresence::Inconclusive),
    ] {
        assert_eq!(StartMenuPresence::from_probe(probe), presence);
        assert_eq!(presence.as_probe(), probe);
    }
}

#[test]
fn an_unseen_display_name_has_no_remembered_answer() {
    let cache = StartMenuCache::default();
    assert_eq!(cache.lookup("Claude"), None);
}

#[test]
fn a_recorded_answer_is_returned_for_the_name_it_was_recorded_under_and_no_other() {
    let cache = StartMenuCache::default();
    cache.record("Claude", StartMenuPresence::Present);

    assert_eq!(cache.lookup("Claude"), Some(StartMenuPresence::Present));
    assert_eq!(cache.lookup("Codex"), None);
}

#[test]
fn recording_the_same_name_again_replaces_the_earlier_answer() {
    let cache = StartMenuCache::default();
    cache.record("Claude", StartMenuPresence::Present);
    cache.record("Claude", StartMenuPresence::Absent);

    assert_eq!(cache.lookup("Claude"), Some(StartMenuPresence::Absent));
}

#[test]
fn an_inconclusive_probe_is_remembered_as_inconclusive_rather_than_as_absent() {
    let cache = StartMenuCache::default();
    cache.record("Claude", StartMenuPresence::from_probe(None));

    assert_eq!(
        cache.lookup("Claude"),
        Some(StartMenuPresence::Inconclusive),
        "a failed query must not be cached as a negative answer"
    );
}

#[test]
fn the_cache_is_shared_across_threads_without_losing_entries() {
    let cache = std::sync::Arc::new(StartMenuCache::default());
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let cache = std::sync::Arc::clone(&cache);
            std::thread::spawn(move || {
                cache.record(&format!("Host {i}"), StartMenuPresence::Present);
            })
        })
        .collect();
    for h in handles {
        h.join().expect("recorder thread");
    }

    for i in 0..8 {
        assert_eq!(
            cache.lookup(&format!("Host {i}")),
            Some(StartMenuPresence::Present)
        );
    }
}
