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

#[test]
fn dropping_a_call_result_is_a_discard() {
    assert_eq!(
        inspect("fn run() { drop(fs::remove_file(path)); }", "discarded")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        inspect("fn run() { drop(client.send().await); }", "discarded")
            .unwrap()
            .len(),
        1
    );
    assert!(
        inspect(
            "fn run() { drop(guard); drop(self.buffer); drop(ManuallyDrop::into_inner(current)); drop(mem::replace(&mut slot, next)); drop(cell.take()); }",
            "discarded"
        )
        .unwrap()
        .is_empty()
    );
    assert!(
        inspect(
            "fn run() {\n // Why: discard-ok: the lock is released before the callback runs\n drop(state.lock());\n}",
            "discarded"
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn awaited_and_named_fallible_receivers_cannot_default_silently() {
    for source in [
        "async fn run() { let body = response.text().await.unwrap_or_default(); }",
        "fn run() { let body = response.text().unwrap_or_default(); }",
        "fn run() { let raw = fs::read_to_string(root).unwrap_or_default(); }",
        "fn run() { let raw = std::fs::read_to_string(root).unwrap_or_default(); }",
        "fn run() { let value = serde_json::to_value(&output).unwrap_or_default(); }",
        "fn run() { let parsed = serde_json::from_slice::<Config>(bytes).unwrap_or_default(); }",
        "fn run() { let count = u64::try_from(delta).unwrap_or_default(); }",
        "fn run() { let user = env::var(\"USER\").unwrap_or_default(); }",
        "fn run() { let user = std::env::var(\"USER\").unwrap_or_default(); }",
        "fn run() { let exe = std::env::current_exe().unwrap_or_default(); }",
        "fn run() { let n = text.parse::<u32>().unwrap_or_default(); }",
        "fn run() { let v = CELL.try_with(|c| c.get()).unwrap_or_default(); }",
        "fn run() { let v = (response.bytes().await).unwrap_or_default(); }",
    ] {
        let findings = inspect(source, "discarded").unwrap();
        assert_eq!(findings.len(), 1, "{source}");
        assert_eq!(findings[0].rule, "fallible-default", "{source}");
    }
}

#[test]
fn option_receivers_and_annotated_defaults_pass() {
    for source in [
        "fn run() { let name = config.name.clone().unwrap_or_default(); }",
        "fn run() { let items = map.get(key).cloned().unwrap_or_default(); }",
        "fn run() { let first = list.first().copied().unwrap_or_default(); }",
        "fn run() { let s = String::from_utf8_lossy(bytes).into_owned(); let t = s.strip_prefix(\"a\").map(str::to_owned).unwrap_or_default(); }",
        "fn run() {\n // Why: discard-ok: USER is a cosmetic label, absence renders an empty owner\n let user = env::var(\"USER\").unwrap_or_default();\n}",
    ] {
        assert!(inspect(source, "discarded").unwrap().is_empty(), "{source}");
    }
}

#[test]
fn tracing_messages_are_constant() {
    for source in [
        "fn run() { tracing::warn!(\"failed to run `lsof -ti :{port}`\"); }",
        "fn run() { info!(\"Discovered {} jobs via inventory, {} configured\", a, b); }",
        "fn run() { error!(error = %e, \"catalog discovery skipped: {e}\"); }",
        "fn run() { tracing::debug!(target: \"boot\", \"{what} failed\"); }",
        "fn run() { info!(\"{count} items\", count = 3); }",
    ] {
        let findings = inspect(source, "tracing-messages").unwrap();
        assert_eq!(findings.len(), 1, "{source}");
        assert_eq!(
            findings[0].rule, "tracing-message-interpolation",
            "{source}"
        );
    }
    for source in [
        "fn run() { tracing::warn!(port = port, \"failed to run lsof\"); }",
        "fn run() { info!(\"{}\", prepared); }",
        "fn run() { info!(\"{message}\"); }",
        "fn run() { warn!(\"literal braces {{}} are fine\"); }",
        "fn run() { error!(message = \"{e}\", \"constant\"); }",
        "fn run() { println!(\"{value} is not tracing\"); }",
        "fn run() { anyhow::bail!(\"{value} is not tracing\"); }",
        "fn run() { tracing::info!(name: \"span {x}\", \"constant\"); }",
        "fn run() { info!(path = %format!(\"{a}/{b}\"), \"constant\"); }",
    ] {
        let findings = inspect(source, "tracing-messages").unwrap();
        assert!(findings.is_empty(), "{source}: {findings:?}");
    }
}
