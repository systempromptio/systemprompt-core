use systemprompt_loader::ModuleLoader;

#[test]
fn test_all_returns_modules() {
    let modules = ModuleLoader::all();
    assert!(!modules.is_empty(), "Should return embedded modules");
}

#[test]
fn test_modules_have_required_fields() {
    let modules = ModuleLoader::all();

    for module in &modules {
        assert!(!module.name.is_empty(), "Module name should not be empty");
        assert!(
            !module.display_name.is_empty(),
            "Module display_name should not be empty"
        );
    }
}

#[test]
fn test_modules_are_sorted_by_weight() {
    let modules = ModuleLoader::all();

    let weights: Vec<i32> = modules
        .iter()
        .map(|m| m.weight.unwrap_or(100))
        .collect();

    let mut sorted_weights = weights.clone();
    sorted_weights.sort();

    assert_eq!(weights, sorted_weights, "Modules should be sorted by weight");
}
