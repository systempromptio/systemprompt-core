//! Which addresses `InstanceAddress::parse` admits, and the origin it keeps.

use systemprompt_mobile::{InstanceAddress, MobileError};

fn origin(input: &str) -> String {
    InstanceAddress::parse(input)
        .expect("an instance address")
        .as_str()
        .to_owned()
}

#[test]
fn https_origin_is_kept_without_a_trailing_slash() {
    assert_eq!(origin("https://demo.systemprompt.io/"), "https://demo.systemprompt.io");
    assert_eq!(origin("  https://demo.systemprompt.io  "), "https://demo.systemprompt.io");
    assert_eq!(origin("https://demo.systemprompt.io:8443"), "https://demo.systemprompt.io:8443");
}

#[test]
fn plain_http_is_admitted_for_loopback_and_private_hosts_only() {
    for local in [
        "http://localhost:8080",
        "http://127.0.0.1:8080",
        "http://10.0.2.2:8080",
        "http://192.168.1.20:8080",
        "http://172.16.4.4",
        "http://[::1]:8080",
    ] {
        assert!(InstanceAddress::parse(local).is_ok(), "{local}");
    }
    for public in ["http://demo.systemprompt.io", "http://8.8.8.8", "http://172.32.0.1"] {
        assert_eq!(InstanceAddress::parse(public), Err(MobileError::InvalidAddress), "{public}");
    }
}

#[test]
fn anything_beyond_an_origin_is_refused() {
    for input in [
        "",
        "demo.systemprompt.io",
        "ftp://demo.systemprompt.io",
        "https://user@demo.systemprompt.io",
        "https://user:secret@demo.systemprompt.io",
        "https://demo.systemprompt.io/admin",
        "https://demo.systemprompt.io/?next=1",
        "https://demo.systemprompt.io/#top",
        "https://",
    ] {
        assert_eq!(InstanceAddress::parse(input), Err(MobileError::InvalidAddress), "{input:?}");
    }
}

#[test]
fn join_appends_a_path_to_the_origin() {
    let address = InstanceAddress::parse("https://demo.systemprompt.io/").expect("an instance address");
    assert_eq!(
        address.join("/v1/bridge/download/android"),
        "https://demo.systemprompt.io/v1/bridge/download/android"
    );
}
