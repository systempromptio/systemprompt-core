use rust_contracts::inspect;

#[test]
fn catches_multiline_and_both_discard_spellings() {
    let source = "fn run() {\n let _ = write(\n target, body);\n _ = save();\n write().ok();\n}";
    assert_eq!(inspect(source, "discarded").unwrap().len(), 3);
}

#[test]
fn annotations_are_local_and_require_a_reason() {
    assert!(inspect("fn run() {\n // Why: discard-ok: obsolete temporary path after failure\n _ = cleanup();\n}", "discarded").unwrap().is_empty());
    assert_eq!(
        inspect(
            "fn run() {\n // Why: discard-ok:\n _ = write();\n}",
            "discarded"
        )
        .unwrap()
        .len(),
        1
    );
}

#[test]
fn parses_guards_in_methods_and_ignores_strings() {
    let source = "impl Guard { fn is_allowed(&self) -> bool { self.value.map_or(\n true, check) } } fn example() { let text = \"_ = write();\"; }";
    assert_eq!(inspect(source, "fail-open").unwrap().len(), 1);
    assert!(inspect(source, "discarded").unwrap().is_empty());
}

#[test]
fn malformed_source_fails_the_scan() {
    assert!(inspect("fn incomplete(", "discarded").is_err());
}

#[test]
fn named_error_erasure_and_trait_guards_are_checked() {
    assert_eq!(
        inspect("fn run() { let value = save().ok(); }", "discarded")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        inspect(
            "trait Guard { fn is_allowed(&self) -> bool { None::<bool>.map_or(true, |b| b) } }",
            "fail-open"
        )
        .unwrap()
        .len(),
        1
    );
    assert!(
        inspect(
            "fn check() { report.ok(\"name\", \"value\"); }",
            "discarded"
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn a_string_cannot_supply_a_cleanup_annotation() {
    let source = r##"fn run() {
        let text = r#"// Why: discard-ok: not a real annotation"#;
        _ = write();
    }"##;
    assert_eq!(inspect(source, "discarded").unwrap().len(), 1);
}

#[test]
fn fallible_side_effect_defaults_and_parse_guards_are_rejected() {
    assert_eq!(
        inspect(
            "fn run() { fs::write(path, bytes).unwrap_or_default(); }",
            "discarded"
        )
        .unwrap()
        .len(),
        1
    );
    assert_eq!(
        inspect(
            "fn is_allowed() -> bool { parse().unwrap_or(true) }",
            "fail-open"
        )
        .unwrap()
        .len(),
        1
    );
}

#[test]
fn a_guard_projecting_an_inventory_must_be_fallible() {
    let projected = "fn allowed_tools(known_tools: &[Tool]) -> Vec<Tool> { known_tools.iter().filter(|t| t.ok).cloned().collect() }";
    let findings = inspect(projected, "fail-open").unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule, "partial-projection");

    let looped = "fn is_allowed(inventory: Inventory) -> bool { for item in inventory { if item.bad { return false; } } true }";
    assert_eq!(
        inspect(looped, "fail-open").unwrap()[0].rule,
        "partial-projection"
    );

    let withheld = "fn allowed_tools(tool_catalog: &Catalog) -> Option<Vec<Tool>> { if tool_catalog.is_empty() { return None; } Some(tool_catalog.iter().cloned().collect()) }";
    assert!(inspect(withheld, "fail-open").unwrap().is_empty());

    let unrelated =
        "fn allowed_tools(tools: &[Tool]) -> Vec<Tool> { tools.iter().cloned().collect() }";
    assert!(inspect(unrelated, "fail-open").unwrap().is_empty());
}
