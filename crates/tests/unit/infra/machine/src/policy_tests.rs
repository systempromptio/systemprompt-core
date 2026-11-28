use systemprompt_machine::policy::{PolicyError, PolicyIdentity, PolicyLocation, locations, read};

fn identity() -> PolicyIdentity {
    PolicyIdentity::new(
        "systemprompt-machine-tests-absent",
        "io.systemprompt.machine-tests.absent",
        "policy.json",
    )
    .expect("a valid identity")
}

#[test]
fn identity_keeps_its_three_names() {
    let identity = identity();
    assert_eq!(identity.name(), "systemprompt-machine-tests-absent");
    assert_eq!(identity.domain(), "io.systemprompt.machine-tests.absent");
    assert_eq!(identity.file_name(), "policy.json");
}

#[test]
fn names_that_would_leave_the_policy_directory_are_refused() {
    let cases = [
        ("../etc", "io.example", "policy.json", "name"),
        ("example", "io/example", "policy.json", "domain"),
        ("example", "io.example", r"..\policy.json", "file name"),
        ("", "io.example", "policy.json", "name"),
        ("example", "io.example", ".hidden", "file name"),
        ("ex ample", "io.example", "policy.json", "name"),
    ];
    for (name, domain, file_name, expected) in cases {
        match PolicyIdentity::new(name, domain, file_name) {
            Err(PolicyError::Identity { field, .. }) => assert_eq!(field, expected),
            other => panic!("{name:?}/{domain:?}/{file_name:?} must be refused, got {other:?}"),
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn linux_reads_one_file_under_etc() {
    let found = locations(&identity()).expect("locations");
    assert_eq!(
        found,
        vec![PolicyLocation::File {
            path: "/etc/systemprompt-machine-tests-absent/policy.json".into()
        }]
    );
}

#[cfg(target_os = "macos")]
#[test]
fn macos_reads_the_user_profile_before_the_machine_profile() {
    use systemprompt_machine::policy::PolicyScope;

    let found = locations(&identity()).expect("locations");
    let scopes: Vec<_> = found
        .iter()
        .map(|location| match location {
            PolicyLocation::ManagedPreferences { path, scope } => {
                assert!(path.starts_with("/Library/Managed Preferences"));
                assert!(path.ends_with("io.systemprompt.machine-tests.absent.plist"));
                *scope
            },
            other => panic!("unexpected location {other:?}"),
        })
        .collect();
    assert_eq!(scopes, vec![PolicyScope::User, PolicyScope::Machine]);
}

#[cfg(windows)]
#[test]
fn windows_reads_the_machine_policy_key_then_a_program_data_file() {
    let found = locations(&identity()).expect("locations");
    assert_eq!(found.len(), 2);
    assert_eq!(
        found[0],
        PolicyLocation::Registry {
            key: r"HKLM\SOFTWARE\Policies\systemprompt-machine-tests-absent".to_owned()
        }
    );
    match &found[1] {
        PolicyLocation::File { path } => {
            assert!(path.is_absolute());
            assert!(path.ends_with(r"systemprompt-machine-tests-absent\policy.json"));
        },
        other => panic!("unexpected location {other:?}"),
    }
}

#[test]
fn a_machine_without_the_policy_reads_nothing() {
    assert_eq!(read(&identity()).expect("read"), Vec::new());
}
