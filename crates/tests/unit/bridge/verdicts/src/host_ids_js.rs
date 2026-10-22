//! The web tree's `host-ids.js` is the one hand-written mirror of `HostKind`.
//! It cannot import the Rust enum, so this test holds the two lists equal, in
//! order, and holds the packaging values the GUI switches its labels on to the
//! two strings `HostAppKind` serialises to.

use systemprompt_models::bridge::host::{HostAppKind, HostKind};

use crate::repo_path;

fn mirrored_ids() -> Vec<String> {
    let source = std::fs::read_to_string(repo_path("bin/bridge/web/js/utils/host-ids.js"))
        .expect("host-ids.js is readable");
    let body = source
        .split_once("HOST_ID = Object.freeze({")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("})"))
        .map(|(body, _)| body)
        .expect("host-ids.js declares HOST_ID as a frozen object literal");
    body.lines()
        .filter_map(|line| {
            let (_, value) = line.split_once(": \"")?;
            let (id, _) = value.split_once('"')?;
            Some(id.to_owned())
        })
        .collect()
}

#[test]
fn host_ids_js_mirrors_host_kind_all_in_order() {
    let rust: Vec<String> = HostKind::ALL
        .iter()
        .map(|kind| kind.as_str().to_owned())
        .collect();
    assert_eq!(
        mirrored_ids(),
        rust,
        "bin/bridge/web/js/utils/host-ids.js has drifted from HostKind::ALL"
    );
}

#[test]
fn the_packaging_wire_values_are_the_two_the_gui_labels() {
    let values: Vec<String> = [HostAppKind::DesktopApp, HostAppKind::CliTool]
        .iter()
        .map(|kind| {
            serde_json::to_value(kind)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(values, ["desktop-app", "cli-tool"]);
    for kind in [HostAppKind::DesktopApp, HostAppKind::CliTool] {
        assert_eq!(serde_json::to_value(kind).unwrap(), kind.to_string());
    }
}
