use systemprompt_bridge::wire::StatePayload;
use systemprompt_bridge::wire::ipc::{
    BridgeError, ErrorCode, ErrorScope, IpcReplyPayload, IpcRequest,
};
use ts_rs::{Config, TS};

#[test]
#[ignore]
fn export_bindings() {
    assert!(
        std::env::var_os("TS_RS_EXPORT_DIR").is_some(),
        "TS_RS_EXPORT_DIR must be set so ts-rs writes paths relative to the crate root. Run: \
         TS_RS_EXPORT_DIR=. cargo test -p systemprompt-bridge-ts-export-tests export_bindings -- \
         --ignored"
    );
    let cfg = Config::from_env();
    BridgeError::export_all(&cfg).expect("export BridgeError");
    ErrorScope::export_all(&cfg).expect("export ErrorScope");
    ErrorCode::export_all(&cfg).expect("export ErrorCode");
    IpcRequest::export_all(&cfg).expect("export IpcRequest");
    IpcReplyPayload::export_all(&cfg).expect("export IpcReplyPayload");
    StatePayload::export_all(&cfg).expect("export StatePayload and everything it carries");
}
