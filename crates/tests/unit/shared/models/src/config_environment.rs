use systemprompt_models::config::{Environment, VerbosityLevel};

#[test]
fn environment_is_dev_prod_test_helpers() {
    assert!(Environment::Development.is_development());
    assert!(!Environment::Development.is_production());
    assert!(!Environment::Development.is_test());

    assert!(Environment::Production.is_production());
    assert!(!Environment::Production.is_development());

    assert!(Environment::Test.is_test());
    assert!(!Environment::Test.is_production());
}

#[test]
fn environment_equality() {
    assert_eq!(Environment::Development, Environment::Development);
    assert_ne!(Environment::Development, Environment::Production);
}

#[test]
fn environment_copy() {
    let e = Environment::Test;
    let e2 = e;
    assert_eq!(e2, Environment::Test);
}

#[test]
fn verbosity_level_from_environment_dev_is_verbose() {
    let v = VerbosityLevel::from_environment(Environment::Development);
    assert_eq!(v, VerbosityLevel::Verbose);
}

#[test]
fn verbosity_level_from_environment_prod_is_quiet() {
    let v = VerbosityLevel::from_environment(Environment::Production);
    assert_eq!(v, VerbosityLevel::Quiet);
}

#[test]
fn verbosity_level_from_environment_test_is_normal() {
    let v = VerbosityLevel::from_environment(Environment::Test);
    assert_eq!(v, VerbosityLevel::Normal);
}

#[test]
fn verbosity_level_is_quiet() {
    assert!(VerbosityLevel::Quiet.is_quiet());
    assert!(!VerbosityLevel::Normal.is_quiet());
    assert!(!VerbosityLevel::Verbose.is_quiet());
    assert!(!VerbosityLevel::Debug.is_quiet());
}

#[test]
fn verbosity_level_is_verbose() {
    assert!(!VerbosityLevel::Quiet.is_verbose());
    assert!(!VerbosityLevel::Normal.is_verbose());
    assert!(VerbosityLevel::Verbose.is_verbose());
    assert!(VerbosityLevel::Debug.is_verbose());
}

#[test]
fn verbosity_level_should_show_verbose() {
    assert!(!VerbosityLevel::Quiet.should_show_verbose());
    assert!(!VerbosityLevel::Normal.should_show_verbose());
    assert!(VerbosityLevel::Verbose.should_show_verbose());
    assert!(VerbosityLevel::Debug.should_show_verbose());
}

#[test]
fn verbosity_level_should_log_to_db() {
    assert!(!VerbosityLevel::Quiet.should_log_to_db());
    assert!(VerbosityLevel::Normal.should_log_to_db());
    assert!(VerbosityLevel::Verbose.should_log_to_db());
    assert!(VerbosityLevel::Debug.should_log_to_db());
}

#[test]
fn verbosity_level_copy() {
    let v = VerbosityLevel::Debug;
    let v2 = v;
    assert_eq!(v2, VerbosityLevel::Debug);
}

// `Environment::detect` reads four process signals in a fixed order. The
// consequential direction is a production deployment detected as development,
// since development relaxes checks production enforces — so both the
// precedence between the signals and the fail-safe default are pinned.
//
// Each test clears every signal before setting the one it is about. Without
// that, an ambient `NODE_ENV` on the machine running the suite decides the
// answer instead of the test. nextest runs one process per test, so these
// mutations reach no other test.
mod detect {
    use systemprompt_models::config::Environment;

    const SIGNALS: [&str; 4] = [
        "SYSTEMPROMPT_ENV",
        "RAILWAY_ENVIRONMENT",
        "NODE_ENV",
        "DOCKER_CONTAINER",
    ];

    fn detect_with(pairs: &[(&str, &str)]) -> Environment {
        for name in SIGNALS {
            unsafe { std::env::remove_var(name) }
        }
        for (name, value) in pairs {
            unsafe { std::env::set_var(name, value) }
        }
        Environment::detect()
    }

    // Why: the explicit variable is the operator's direct statement of intent.
    // Any inferred signal outranking it means a deployment cannot be overridden.
    #[test]
    fn the_explicit_variable_outranks_every_other_signal() {
        assert_eq!(
            detect_with(&[
                ("SYSTEMPROMPT_ENV", "development"),
                ("RAILWAY_ENVIRONMENT", "production"),
                ("NODE_ENV", "production"),
                ("DOCKER_CONTAINER", "1"),
            ]),
            Environment::Development,
            "SYSTEMPROMPT_ENV must win over the inferred signals"
        );
    }

    #[test]
    fn the_platform_variable_is_consulted_before_node_env() {
        assert_eq!(
            detect_with(&[
                ("RAILWAY_ENVIRONMENT", "production"),
                ("NODE_ENV", "development"),
            ]),
            Environment::Production,
            "a platform saying production outranks NODE_ENV"
        );
    }

    #[test]
    fn node_env_is_consulted_when_no_stronger_signal_is_present() {
        assert_eq!(detect_with(&[("NODE_ENV", "test")]), Environment::Test);
    }

    // Why: a container that detected development would relax checks wherever
    // it was deployed.
    #[test]
    fn running_in_a_container_is_treated_as_production() {
        assert_eq!(
            detect_with(&[("DOCKER_CONTAINER", "1")]),
            Environment::Production
        );
    }

    // Why: this is the fail-safe. A value nobody recognises must land on the
    // stricter environment — reading an unknown string as development would
    // relax checks because of a typo.
    #[test]
    fn an_unrecognised_value_falls_back_to_production_rather_than_development() {
        for unknown in ["staging", "prod", "", "developmnet"] {
            assert_eq!(
                detect_with(&[("SYSTEMPROMPT_ENV", unknown)]),
                Environment::Production,
                "{unknown:?} is not recognised and must not relax anything"
            );
        }
    }

    // Why: an operator writes `DEV` or `Development` as readily as `dev`. A
    // case-sensitive match sends them to production silently.
    #[test]
    fn environment_names_are_matched_without_regard_to_case() {
        for spelling in ["development", "Development", "DEV", "dev"] {
            assert_eq!(
                detect_with(&[("SYSTEMPROMPT_ENV", spelling)]),
                Environment::Development,
                "{spelling} should name development"
            );
        }

        for spelling in ["test", "Testing", "TEST"] {
            assert_eq!(
                detect_with(&[("SYSTEMPROMPT_ENV", spelling)]),
                Environment::Test,
                "{spelling} should name test"
            );
        }
    }

    // Why: the platform variable means production only when it says so. Any
    // other value must fall through to the signals below rather than being
    // read as a production marker.
    #[test]
    fn a_platform_variable_that_is_not_production_falls_through() {
        assert_eq!(
            detect_with(&[
                ("RAILWAY_ENVIRONMENT", "staging"),
                ("NODE_ENV", "development"),
            ]),
            Environment::Development,
            "a non-production platform value must not short-circuit to production"
        );
    }
}
