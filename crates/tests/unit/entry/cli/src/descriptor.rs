#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

use systemprompt_cli::descriptor::{CommandDescriptor, DescribeCommand};

#[test]
fn test_descriptor_none_all_false() {
    let desc = CommandDescriptor::NONE;
    assert!(!desc.profile);
    assert!(!desc.secrets);
    assert!(!desc.paths);
    assert!(!desc.database);
    assert!(!desc.remote_eligible);
    assert!(!desc.skip_validation);
}

#[test]
fn test_descriptor_profile_only() {
    let desc = CommandDescriptor::PROFILE_ONLY;
    assert!(desc.profile);
    assert!(!desc.secrets);
    assert!(!desc.paths);
    assert!(!desc.database);
    assert!(!desc.remote_eligible);
}

#[test]
fn test_descriptor_profile_and_secrets() {
    let desc = CommandDescriptor::PROFILE_AND_SECRETS;
    assert!(desc.profile);
    assert!(desc.secrets);
    assert!(!desc.paths);
    assert!(!desc.database);
}

#[test]
fn test_descriptor_profile_secrets_and_paths() {
    let desc = CommandDescriptor::PROFILE_SECRETS_AND_PATHS;
    assert!(desc.profile);
    assert!(desc.secrets);
    assert!(desc.paths);
    assert!(!desc.database);
}

#[test]
fn test_descriptor_full() {
    let desc = CommandDescriptor::FULL;
    assert!(desc.profile);
    assert!(desc.secrets);
    assert!(desc.paths);
    assert!(desc.database);
    assert!(desc.remote_eligible);
    assert!(!desc.skip_validation);
}

#[test]
fn test_descriptor_default_all_false() {
    let desc = CommandDescriptor::default();
    assert!(!desc.profile);
    assert!(!desc.secrets);
    assert!(!desc.paths);
    assert!(!desc.database);
    assert!(!desc.remote_eligible);
    assert!(!desc.skip_validation);
}

#[test]
fn test_descriptor_debug_format() {
    let desc = CommandDescriptor::FULL;
    let debug = format!("{:?}", desc);
    assert!(debug.contains("CommandDescriptor"));
    assert!(debug.contains("profile"));
    assert!(debug.contains("remote_eligible"));
}

#[test]
fn test_descriptor_with_remote_eligible() {
    let desc = CommandDescriptor::PROFILE_ONLY.with_remote_eligible();
    assert!(desc.profile);
    assert!(!desc.secrets);
    assert!(desc.remote_eligible);
}

#[test]
fn test_descriptor_with_skip_validation() {
    let desc = CommandDescriptor::FULL.with_skip_validation();
    assert!(desc.profile);
    assert!(desc.database);
    assert!(desc.remote_eligible);
    assert!(desc.skip_validation);
}

struct TestCommand {
    use_database: bool,
}

impl DescribeCommand for TestCommand {
    fn descriptor(&self) -> CommandDescriptor {
        if self.use_database {
            CommandDescriptor::FULL
        } else {
            CommandDescriptor::PROFILE_ONLY
        }
    }
}

#[test]
fn test_describe_command_trait_full() {
    let cmd = TestCommand { use_database: true };
    let desc = cmd.descriptor();
    assert!(desc.database);
    assert!(desc.profile);
    assert!(desc.remote_eligible);
}

#[test]
fn test_describe_command_trait_profile_only() {
    let cmd = TestCommand { use_database: false };
    let desc = cmd.descriptor();
    assert!(!desc.database);
    assert!(desc.profile);
    assert!(!desc.remote_eligible);
}

#[test]
fn test_descriptor_none_vs_default() {
    let none = CommandDescriptor::NONE;
    let default = CommandDescriptor::default();
    assert_eq!(none.profile, default.profile);
    assert_eq!(none.secrets, default.secrets);
    assert_eq!(none.paths, default.paths);
    assert_eq!(none.database, default.database);
    assert_eq!(none.remote_eligible, default.remote_eligible);
    assert_eq!(none.skip_validation, default.skip_validation);
}

#[test]
fn test_descriptor_hierarchy_none_to_full() {
    let none = CommandDescriptor::NONE;
    let profile_only = CommandDescriptor::PROFILE_ONLY;
    let profile_and_secrets = CommandDescriptor::PROFILE_AND_SECRETS;
    let profile_secrets_paths = CommandDescriptor::PROFILE_SECRETS_AND_PATHS;
    let full = CommandDescriptor::FULL;

    assert!(!none.profile);
    assert!(profile_only.profile && !profile_only.secrets);
    assert!(profile_and_secrets.profile && profile_and_secrets.secrets && !profile_and_secrets.paths);
    assert!(profile_secrets_paths.profile && profile_secrets_paths.secrets && profile_secrets_paths.paths && !profile_secrets_paths.database);
    assert!(full.profile && full.secrets && full.paths && full.database);
}
