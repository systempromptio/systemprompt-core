use systemprompt_bridge::ids::PinnedPubKey;
use systemprompt_bridge::install::{InstallOptions, InstallOptionsBuilder};
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
    assert!(!opts.apply_schedule);
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
    assert!(!opts.apply_schedule);
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
    let url = ValidatedUrl::try_new("https://gw.example.com").expect("valid ValidatedUrl");
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
        .gateway_url(ValidatedUrl::try_new("https://gw.example.com").expect("valid ValidatedUrl"))
        .pubkey(PinnedPubKey::new("base64data"))
        .apply(true)
        .apply_mobileconfig(true)
        .apply_schedule(true)
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
    assert!(opts.apply_schedule);
}

#[test]
fn apply_schedule_setter_sets_field() {
    let opts = InstallOptions::builder().apply_schedule(true).build();
    assert!(opts.apply_schedule);
}
