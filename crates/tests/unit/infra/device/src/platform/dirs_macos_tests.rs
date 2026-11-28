//! The Application Support directories.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use systemprompt_device::platform::{DirsError, Environment, local_data_dir, machine_data_dir};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn unset(_name: &str) -> Option<OsString> {
    None
}

#[test]
fn the_local_data_directory_is_the_users_application_support() -> TestResult {
    let environment = Environment::new(Some(PathBuf::from("/Users/user")), &unset);
    assert_eq!(
        local_data_dir(&environment)?,
        Path::new("/Users/user/Library/Application Support")
    );
    Ok(())
}

#[test]
fn without_a_home_there_is_no_local_data_directory() {
    let environment = Environment::new(None, &unset);
    assert!(matches!(
        local_data_dir(&environment),
        Err(DirsError::NoHome)
    ));
}

#[test]
fn the_machine_data_directory_is_the_systems_application_support() -> TestResult {
    let environment = Environment::new(None, &unset);
    assert_eq!(
        machine_data_dir(&environment)?,
        Path::new("/Library/Application Support")
    );
    Ok(())
}
