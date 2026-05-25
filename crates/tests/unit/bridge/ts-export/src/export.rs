#[cfg(any(target_os = "windows", target_os = "macos"))]
use systemprompt_bridge::gui::ipc::{
    BridgeError, ErrorCode, ErrorScope, IpcReplyPayload, IpcRequest,
};
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use systemprompt_bridge::ipc_types::{
    BridgeError, ErrorCode, ErrorScope, IpcReplyPayload, IpcRequest,
};
use ts_rs::TS;

#[test]
#[ignore]
fn export_bindings() {
    assert!(
        std::env::var_os("TS_RS_EXPORT_DIR").is_some(),
        "TS_RS_EXPORT_DIR must be set so ts-rs writes paths relative to the crate root. Run: \
         TS_RS_EXPORT_DIR=. cargo test -p systemprompt-bridge-ts-export-tests export_bindings -- --ignored"
    );
    BridgeError::export_all().expect("export BridgeError");
    ErrorScope::export_all().expect("export ErrorScope");
    ErrorCode::export_all().expect("export ErrorCode");
    IpcRequest::export_all().expect("export IpcRequest");
    IpcReplyPayload::export_all().expect("export IpcReplyPayload");
}
