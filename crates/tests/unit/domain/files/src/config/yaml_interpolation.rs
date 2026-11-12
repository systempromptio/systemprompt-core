//! `files.yaml` resolves `${VAR}` placeholders the way a profile does.

use std::path::Path;

use systemprompt_files::{FilesConfig, FilesError};

const YAML: &str = "files:\n  documents:\n    collab:\n      owner:\n        enabled: true\n        advertiseHost: HOST\n";

fn parse(
    host: &str,
    lookup: &impl Fn(&str) -> Option<String>,
) -> Result<Option<String>, FilesError> {
    let yaml = YAML.replace("HOST", host);
    FilesConfig::parse_yaml(&yaml, Path::new("files.yaml"), lookup)
        .map(|config| config.documents.collab.owner.advertise_host)
}

#[test]
fn set_variable_supplies_the_advertised_host() {
    let host = parse("${FLY_PRIVATE_IP:-}", &|name| {
        (name == "FLY_PRIVATE_IP").then(|| "fdaa::3".to_owned())
    })
    .expect("parse");
    assert_eq!(host.as_deref(), Some("fdaa::3"));
}

#[test]
fn empty_default_leaves_the_host_empty() {
    let host = parse("${FLY_PRIVATE_IP:-}", &|_| None).expect("parse");
    assert!(host.as_deref().is_none_or(str::is_empty), "got {host:?}");
}

#[test]
fn unset_variable_without_default_is_an_interpolation_error() {
    let err = parse("${POD_IP}", &|_| None).expect_err("unset variable must fail");
    match err {
        FilesError::ConfigInterpolation { source, .. } => {
            assert_eq!(source.variable, "POD_IP");
            assert_eq!(
                source.key_path,
                "files.documents.collab.owner.advertiseHost"
            );
        },
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn literal_host_is_kept() {
    let host = parse("10.0.0.7", &|_| None).expect("parse");
    assert_eq!(host.as_deref(), Some("10.0.0.7"));
}
