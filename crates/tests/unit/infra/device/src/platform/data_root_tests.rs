use std::ffi::OsString;
use std::path::PathBuf;

use systemprompt_device::platform::{
    DataRoot, DirsError, Environment, RootName, home_dir, local_data_dir,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn unset(_name: &str) -> Option<OsString> {
    None
}

#[test]
fn root_name_is_the_vendor_then_the_product_with_its_channel() -> TestResult {
    let stable = RootName::new("Example", "Product", None)?;
    assert_eq!(stable.vendor(), "Example");
    assert_eq!(stable.product(), "Product");
    let beta = RootName::new("Example", "Product", Some("Beta"))?;
    assert_eq!(beta.product(), "Product Beta");
    assert_ne!(stable, beta);
    Ok(())
}

#[test]
fn root_name_refuses_a_part_that_is_not_a_portable_file_name() {
    for bad in [
        "", ".", "..", "a/b", "a\\b", "a:b", "CON", "name.", " name", "name ",
    ] {
        assert!(
            matches!(
                RootName::new(bad, "Product", None),
                Err(DirsError::Name { .. })
            ),
            "vendor {bad:?}"
        );
        assert!(
            matches!(
                RootName::new("Example", bad, None),
                Err(DirsError::Name { .. })
            ),
            "product {bad:?}"
        );
        assert!(
            matches!(
                RootName::new("Example", "Product", Some(bad)),
                Err(DirsError::Name { .. })
            ),
            "channel {bad:?}"
        );
    }
}

#[test]
fn data_root_has_one_directory_per_kind() {
    let root = PathBuf::from("base").join("Example").join("Product");
    let layout = DataRoot::at(root.clone());
    assert_eq!(layout.root(), root.as_path());
    assert_eq!(layout.data(), root.join("data"));
    assert_eq!(layout.config(), root.join("config"));
    assert_eq!(layout.cache(), root.join("cache"));
    assert_eq!(layout.state(), root.join("state"));
    assert_eq!(layout.tmp(), root.join("tmp"));
}

#[test]
fn the_user_root_is_the_root_name_under_the_local_data_directory() -> TestResult {
    let base = tempfile::tempdir()?;
    let answer = base.path().as_os_str().to_owned();
    let variables = move |_name: &str| Some(answer.clone());
    let environment = Environment::new(Some(base.path().to_path_buf()), &variables);
    let name = RootName::new("Example", "Product", Some("Beta"))?;
    let layout = DataRoot::for_user(&environment, &name)?;
    assert_eq!(
        layout.root(),
        local_data_dir(&environment)?
            .join("Example")
            .join("Product Beta")
    );
    assert!(layout.root().starts_with(base.path()));
    Ok(())
}

#[test]
fn ensure_creates_every_directory_and_is_repeatable() -> TestResult {
    let base = tempfile::tempdir()?;
    let layout = DataRoot::at(base.path().join("Example").join("Product"));
    layout.ensure()?;
    layout.ensure()?;
    for dir in [
        layout.data(),
        layout.config(),
        layout.cache(),
        layout.state(),
        layout.tmp(),
    ] {
        assert!(dir.is_dir(), "{}", dir.display());
    }
    Ok(())
}

#[test]
fn home_must_be_known_and_absolute() -> TestResult {
    let base = tempfile::tempdir()?;
    let known = Environment::new(Some(base.path().to_path_buf()), &unset);
    assert_eq!(home_dir(&known)?, base.path());
    let unknown = Environment::new(None, &unset);
    assert!(matches!(home_dir(&unknown), Err(DirsError::NoHome)));
    let relative = Environment::new(Some(PathBuf::from("home")), &unset);
    assert!(matches!(home_dir(&relative), Err(DirsError::NoHome)));
    Ok(())
}

#[test]
fn the_process_environment_is_a_usable_environment() {
    let environment = Environment::process();
    assert!(format!("{environment:?}").contains("home"));
}
