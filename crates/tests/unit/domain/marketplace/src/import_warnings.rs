use systemprompt_marketplace::ImportWarning;

fn all_variants() -> Vec<ImportWarning> {
    vec![
        ImportWarning::NoMarketplaceManifest,
        ImportWarning::InlineMcpServers {
            plugin: "alpha".to_owned(),
        },
        ImportWarning::CommandsDirectory {
            plugin: "alpha".to_owned(),
        },
        ImportWarning::AgentsDirectory {
            plugin: "alpha".to_owned(),
            count: 3,
        },
        ImportWarning::MissingCategory {
            plugin: "alpha".to_owned(),
            applied: "general".to_owned(),
        },
        ImportWarning::UnsupportedHookAction {
            plugin: "alpha".to_owned(),
            event: "PreToolUse".to_owned(),
        },
        ImportWarning::RemotePluginSource {
            plugin: "alpha".to_owned(),
        },
        ImportWarning::NoSkills {
            plugin: "alpha".to_owned(),
        },
        ImportWarning::UnattachedRootRules {
            rules: vec!["security".to_owned(), "handover".to_owned()],
        },
    ]
}

#[test]
fn every_warning_names_what_it_is_about() {
    for warning in all_variants() {
        let text = warning.to_string();
        assert!(text.len() > 20, "{warning:?} renders as '{text}'");
        match &warning {
            ImportWarning::NoMarketplaceManifest => {
                assert!(text.contains("marketplace.json"), "{text}");
            },
            ImportWarning::UnattachedRootRules { .. } => {
                assert!(text.contains("security, handover"), "{text}");
            },
            _ => assert!(text.contains("alpha"), "{text}"),
        }
    }
}

#[test]
fn each_warning_explains_the_untranslatable_fact() {
    let rendered: Vec<String> = all_variants().iter().map(ToString::to_string).collect();

    assert!(rendered[1].contains("referenced by id"), "{}", rendered[1]);
    assert!(rendered[2].contains("commands/"), "{}", rendered[2]);
    assert!(rendered[3].contains("3 agent file(s)"), "{}", rendered[3]);
    assert!(
        rendered[4].contains("'general' was applied"),
        "{}",
        rendered[4]
    );
    assert!(rendered[5].contains("PreToolUse"), "{}", rendered[5]);
    assert!(rendered[6].contains("non-local source"), "{}", rendered[6]);
    assert!(rendered[7].contains("no skills"), "{}", rendered[7]);
}

#[test]
fn only_untranslatable_facts_are_strict_errors() {
    let strict: Vec<bool> = all_variants()
        .iter()
        .map(ImportWarning::is_strict_error)
        .collect();

    assert_eq!(
        strict,
        vec![true, true, true, false, true, false, true, false, true]
    );
}

#[test]
fn a_warning_compares_by_its_payload() {
    let a = ImportWarning::NoSkills {
        plugin: "alpha".to_owned(),
    };
    let b = ImportWarning::NoSkills {
        plugin: "beta".to_owned(),
    };

    assert_eq!(a, a.clone());
    assert_ne!(a, b);
    assert_ne!(a, ImportWarning::NoMarketplaceManifest);
}
