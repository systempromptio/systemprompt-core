//! The XDG base directories.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use systemprompt_device::platform::{DirsError, Environment, local_data_dir, machine_data_dir};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn unset(_name: &str) -> Option<OsString> {
    None
}

#[test]
fn an_absolute_xdg_data_home_is_the_local_data_directory() -> TestResult {
    let variables =
        |name: &str| (name == "XDG_DATA_HOME").then(|| OsString::from("/srv/data-home"));
    let environment = Environment::new(Some(PathBuf::from("/home/user")), &variables);
    assert_eq!(local_data_dir(&environment)?, Path::new("/srv/data-home"));
    Ok(())
}

#[test]
fn a_relative_or_unset_xdg_data_home_falls_back_to_the_home_directory() -> TestResult {
    let relative = |name: &str| (name == "XDG_DATA_HOME").then(|| OsString::from("data-home"));
    let environment = Environment::new(Some(PathBuf::from("/home/user")), &relative);
    assert_eq!(
        local_data_dir(&environment)?,
        Path::new("/home/user/.local/share")
    );
    let environment = Environment::new(Some(PathBuf::from("/home/user")), &unset);
    assert_eq!(
        local_data_dir(&environment)?,
        Path::new("/home/user/.local/share")
    );
    Ok(())
}

#[test]
fn without_a_home_or_a_data_home_there_is_no_local_data_directory() {
    let environment = Environment::new(None, &unset);
    assert!(matches!(
        local_data_dir(&environment),
        Err(DirsError::NoHome)
    ));
}

#[test]
fn the_machine_data_directory_is_var_lib() -> TestResult {
    let environment = Environment::new(None, &unset);
    assert_eq!(machine_data_dir(&environment)?, Path::new("/var/lib"));
    Ok(())
}
