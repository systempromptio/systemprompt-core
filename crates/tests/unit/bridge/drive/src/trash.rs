use chrono::{Duration, TimeZone, Utc};
use systemprompt_bridge::drive::trash::{RETENTION_DAYS, move_to_trash, prune, stamp_of};

use crate::support::Fixture;

#[test]
fn trash_older_than_the_retention_is_removed_and_newer_is_kept() {
    let fx = Fixture::new("http://127.0.0.1:9");
    let now = Utc.with_ymd_and_hms(2026, 11, 10, 12, 0, 0).unwrap();
    fx.write("old.md", b"old");
    fx.write("new.md", b"new");
    let old = move_to_trash(&fx.root, "old.md", now - Duration::days(RETENTION_DAYS + 1))
        .expect("trash")
        .expect("moved");
    let new = move_to_trash(&fx.root, "new.md", now - Duration::days(RETENTION_DAYS - 1))
        .expect("trash")
        .expect("moved");
    let foreign = fx.root.trash_dir().join("kept by the user");
    std::fs::create_dir_all(&foreign).expect("foreign");

    assert_eq!(prune(&fx.root, now).expect("prune"), 1);

    assert!(!old.exists());
    assert_eq!(std::fs::read(&new).expect("kept"), b"new");
    assert!(
        foreign.is_dir(),
        "a folder that is not a stamp is never removed"
    );
}

#[test]
fn a_stamp_round_trips_through_its_folder_name() {
    let fx = Fixture::new("http://127.0.0.1:9");
    let at = Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap();
    fx.write("a.md", b"a");
    let moved = move_to_trash(&fx.root, "a.md", at)
        .expect("trash")
        .expect("moved");
    let stamp = moved
        .strip_prefix(fx.root.trash_dir())
        .expect("under trash")
        .components()
        .next()
        .expect("stamp")
        .as_os_str()
        .to_str()
        .expect("utf8")
        .to_owned();
    assert_eq!(stamp_of(&stamp), Some(at));
    assert_eq!(stamp_of("not a stamp"), None);
}
