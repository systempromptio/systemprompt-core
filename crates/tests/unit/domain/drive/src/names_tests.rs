use systemprompt_drive::sync::{conflict_name, is_ignored};

#[test]
fn conflict_copies_keep_folder_and_extension() {
    assert_eq!(
        conflict_name("Reports/q3.md", "DESKTOP-1", 1),
        "Reports/q3 (conflict from DESKTOP-1).md"
    );
    assert_eq!(
        conflict_name("notes", "laptop", 2),
        "notes (conflict from laptop) 2"
    );
    assert_eq!(
        conflict_name(".env", "laptop", 1),
        ".env (conflict from laptop)"
    );
    assert_eq!(
        conflict_name("a.tar.gz", "my:pc", 3),
        "a.tar (conflict from my-pc) 3.gz"
    );
    assert_eq!(
        conflict_name("x.md", "", 1),
        "x (conflict from this device).md"
    );
}

#[test]
fn editor_and_partial_files_are_ignored() {
    let ignored = [
        ".drive-state.json",
        ".trash",
        "~$report.docx",
        "notes.md~",
        "file.tmp",
        "film.crdownload",
        "#draft#",
        "Thumbs.db",
        "x.PART",
    ];
    for name in ignored {
        assert!(is_ignored(name), "{name:?} was synced");
    }
    for name in ["report.docx", "notes.md", "tmp.md", "a~b.txt"] {
        assert!(!is_ignored(name), "{name:?} was ignored");
    }
}
