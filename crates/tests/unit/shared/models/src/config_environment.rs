use systemprompt_models::config::Environment;
use systemprompt_models::config::VerbosityLevel;

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
