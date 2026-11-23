//! `files.documents.externalSharing`: off by default, deny lists, the link
//! lifetime ceiling.

use systemprompt_files::{DocumentsConfig, ExternalChannel, ExternalSharingConfig};
use systemprompt_identifiers::DocumentFolderId;
use systemprompt_models::documents::DocumentFormat;

fn folder(n: u8) -> DocumentFolderId {
    DocumentFolderId::new(format!("00000000-0000-4000-8000-0000000000{n:02}"))
}

fn open() -> ExternalSharingConfig {
    ExternalSharingConfig {
        public_links: true,
        invites: true,
        ..ExternalSharingConfig::default()
    }
}

#[test]
fn external_sharing_is_off_unless_configured() {
    let config = DocumentsConfig::default().external_sharing;
    for channel in [ExternalChannel::PublicLink, ExternalChannel::Invite] {
        assert!(!config.allows(channel, DocumentFormat::Markdown, &[]));
    }
}

#[test]
fn each_channel_is_switched_on_its_own() {
    let config = ExternalSharingConfig {
        public_links: true,
        ..ExternalSharingConfig::default()
    };
    assert!(config.allows(ExternalChannel::PublicLink, DocumentFormat::Markdown, &[]));
    assert!(!config.allows(ExternalChannel::Invite, DocumentFormat::Markdown, &[]));
}

#[test]
fn a_denied_format_is_withheld() {
    let config = ExternalSharingConfig {
        deny_formats: vec![DocumentFormat::Html],
        ..open()
    };
    assert!(!config.allows(ExternalChannel::PublicLink, DocumentFormat::Html, &[]));
    assert!(config.allows(ExternalChannel::PublicLink, DocumentFormat::Docx, &[]));
}

#[test]
fn a_folder_deny_covers_its_whole_subtree() {
    let config = ExternalSharingConfig {
        deny_folders: vec![folder(1)],
        ..open()
    };
    let beneath = [folder(1), folder(2), folder(3)];
    assert!(!config.allows(ExternalChannel::Invite, DocumentFormat::Text, &beneath));
    assert!(config.allows(ExternalChannel::Invite, DocumentFormat::Text, &[folder(2)]));
}

#[test]
fn a_link_lifetime_never_exceeds_the_ceiling() {
    let config = ExternalSharingConfig {
        max_link_days: 7,
        ..open()
    };
    assert_eq!(config.link_days(None), 7);
    assert_eq!(config.link_days(Some(90)), 7);
    assert_eq!(config.link_days(Some(0)), 1);
    assert_eq!(config.link_days(Some(3)), 3);
}

#[test]
fn the_section_reads_from_camel_case_yaml_and_refuses_unknown_keys() {
    let parsed: ExternalSharingConfig =
        serde_yaml::from_str("publicLinks: true\ndenyFormats: [html]\nmaxLinkDays: 14\n")
            .expect("parse");
    assert!(parsed.public_links && !parsed.invites);
    assert_eq!(parsed.deny_formats, vec![DocumentFormat::Html]);
    assert_eq!(parsed.max_link_days, 14);
    assert!(serde_yaml::from_str::<ExternalSharingConfig>("publicLink: true\n").is_err());
}
