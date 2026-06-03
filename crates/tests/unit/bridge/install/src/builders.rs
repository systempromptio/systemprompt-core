use systemprompt_bridge::ids::PinnedPubKey;
use systemprompt_bridge::install::{
    CredentialsOutcome, InstallOptions, InstallOptionsBuilder, ManagedProfileOutcome,
    UninstallSummaryBuilder,
};
use systemprompt_bridge::schedule::Os;
use systemprompt_identifiers::ValidatedUrl;

#[test]
fn builder_defaults_are_empty() {
    let opts = InstallOptions::builder().build();
    assert!(opts.print_mdm.is_none());
    assert!(opts.emit_schedule_template.is_none());
    assert!(opts.gateway_url.is_none());
    assert!(opts.pubkey.is_none());
    assert!(!opts.apply);
    assert!(!opts.apply_mobileconfig);
}

#[test]
fn builder_new_matches_builder_fn() {
    let opts = InstallOptionsBuilder::new().build();
    assert!(opts.print_mdm.is_none());
    assert!(opts.emit_schedule_template.is_none());
    assert!(opts.gateway_url.is_none());
    assert!(opts.pubkey.is_none());
    assert!(!opts.apply);
    assert!(!opts.apply_mobileconfig);
}

#[test]
fn print_mdm_setter_sets_field() {
    let opts = InstallOptions::builder().print_mdm(Os::Mac).build();
    assert!(matches!(opts.print_mdm, Some(Os::Mac)));
}

#[test]
fn emit_schedule_template_setter_sets_field() {
    let opts = InstallOptions::builder()
        .emit_schedule_template(Os::Linux)
        .build();
    assert!(matches!(opts.emit_schedule_template, Some(Os::Linux)));
}

#[test]
fn gateway_url_setter_sets_field() {
    let url = ValidatedUrl::new("https://gw.example.com");
    let opts = InstallOptions::builder().gateway_url(url).build();
    assert_eq!(
        opts.gateway_url.as_ref().map(ValidatedUrl::as_str),
        Some("https://gw.example.com")
    );
}

#[test]
fn pubkey_setter_sets_field() {
    let opts = InstallOptions::builder()
        .pubkey(PinnedPubKey::new("base64data"))
        .build();
    assert_eq!(
        opts.pubkey.as_ref().map(PinnedPubKey::as_str),
        Some("base64data")
    );
}

#[test]
fn apply_setter_sets_field() {
    let opts = InstallOptions::builder().apply(true).build();
    assert!(opts.apply);
}

#[test]
fn apply_mobileconfig_setter_sets_field() {
    let opts = InstallOptions::builder().apply_mobileconfig(true).build();
    assert!(opts.apply_mobileconfig);
}

#[test]
fn all_setters_chain_together() {
    let opts = InstallOptions::builder()
        .print_mdm(Os::Windows)
        .emit_schedule_template(Os::Mac)
        .gateway_url(ValidatedUrl::new("https://gw.example.com"))
        .pubkey(PinnedPubKey::new("base64data"))
        .apply(true)
        .apply_mobileconfig(true)
        .build();
    assert!(matches!(opts.print_mdm, Some(Os::Windows)));
    assert!(matches!(opts.emit_schedule_template, Some(Os::Mac)));
    assert_eq!(
        opts.gateway_url.as_ref().map(ValidatedUrl::as_str),
        Some("https://gw.example.com")
    );
    assert_eq!(
        opts.pubkey.as_ref().map(PinnedPubKey::as_str),
        Some("base64data")
    );
    assert!(opts.apply);
    assert!(opts.apply_mobileconfig);
}

#[test]
fn uninstall_summary_builder_defaults() {
    let summary = UninstallSummaryBuilder::new().build();
    assert!(summary.metadata_removed.is_none());
    assert!(summary.metadata_already_clean.is_none());
    assert!(matches!(
        summary.managed_profile,
        ManagedProfileOutcome::NotApplicable
    ));
    assert!(matches!(summary.credentials, CredentialsOutcome::Kept));
}

#[test]
fn uninstall_summary_builder_chains_setters() {
    let summary = UninstallSummaryBuilder::new()
        .metadata_removed(std::path::PathBuf::from("/x"))
        .metadata_already_clean(std::path::PathBuf::from("/y"))
        .managed_profile(ManagedProfileOutcome::Removed("profile-id"))
        .credentials(CredentialsOutcome::Purged(std::path::PathBuf::from(
            "/creds",
        )))
        .build();
    assert_eq!(
        summary.metadata_removed,
        Some(std::path::PathBuf::from("/x"))
    );
    assert_eq!(
        summary.metadata_already_clean,
        Some(std::path::PathBuf::from("/y"))
    );
    assert!(matches!(
        summary.managed_profile,
        ManagedProfileOutcome::Removed("profile-id")
    ));
    assert!(matches!(
        summary.credentials,
        CredentialsOutcome::Purged(_)
    ));
}
