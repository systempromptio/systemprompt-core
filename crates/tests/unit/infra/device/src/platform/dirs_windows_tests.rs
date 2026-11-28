//! The `%LOCALAPPDATA%` and `%ProgramData%` directories.

use std::ffi::OsString;
use std::path::Path;

use systemprompt_device::platform::{DirsError, Environment, local_data_dir, machine_data_dir};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn unset(_name: &str) -> Option<OsString> {
    None
}

fn defined(name: &str) -> Option<OsString> {
    match name {
        "LOCALAPPDATA" => Some(OsString::from(r"C:\Users\user\AppData\Local")),
        "ProgramData" => Some(OsString::from(r"C:\ProgramData")),
        _ => None,
    }
}

fn relative(_name: &str) -> Option<OsString> {
    Some(OsString::from("AppData"))
}

#[test]
fn the_local_data_directory_is_localappdata() -> TestResult {
    let environment = Environment::new(None, &defined);
    assert_eq!(
        local_data_dir(&environment)?,
        Path::new(r"C:\Users\user\AppData\Local")
    );
    Ok(())
}

#[test]
fn the_machine_data_directory_is_programdata() -> TestResult {
    let environment = Environment::new(None, &defined);
    assert_eq!(
        machine_data_dir(&environment)?,
        Path::new(r"C:\ProgramData")
    );
    Ok(())
}

#[test]
fn an_unset_or_relative_variable_names_no_directory() {
    let cases: [&dyn Fn(&str) -> Option<OsString>; 2] = [&unset, &relative];
    for variables in cases {
        let environment = Environment::new(None, variables);
        assert!(matches!(
            local_data_dir(&environment),
            Err(DirsError::NotAbsolute {
                variable: "LOCALAPPDATA"
            })
        ));
        assert!(matches!(
            machine_data_dir(&environment),
            Err(DirsError::NotAbsolute {
                variable: "ProgramData"
            })
        ));
    }
}
