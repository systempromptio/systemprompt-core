use systemprompt_bridge::drive::names::{PathRefusal, case_collision, fold, portable_segment};

#[test]
fn composed_and_decomposed_forms_of_one_name_collide() {
    let composed = "notes/caf\u{e9}.md";
    let decomposed = "notes/cafe\u{301}.md";
    assert_eq!(fold(composed), fold(decomposed));
    assert_eq!(
        case_collision(decomposed, [composed]),
        Err(PathRefusal::CaseCollision {
            existing: composed.to_owned()
        })
    );
    assert_eq!(
        case_collision("notes/CAFE\u{301}.md", [composed]),
        Err(PathRefusal::CaseCollision {
            existing: composed.to_owned()
        })
    );
}

#[test]
fn console_and_superscript_device_names_are_not_portable() {
    for name in [
        "CONIN$",
        "conout$.txt",
        "COM\u{b9}",
        "lpt\u{b2}.md",
        "com\u{b3}",
    ] {
        assert_eq!(
            portable_segment(name),
            Err(PathRefusal::ReservedName),
            "{name}"
        );
    }
    for name in ["COM0", "console.md", "com10", "conin.md"] {
        assert_eq!(portable_segment(name), Ok(()), "{name}");
    }
}
