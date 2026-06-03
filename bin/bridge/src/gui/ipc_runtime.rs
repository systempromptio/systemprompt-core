use crate::gui::GuiApp;
use crate::gui::command::{self, CommandOutcome};
use crate::gui::emit::send_reply_payload;
use crate::gui::ipc::IpcRequest;

pub(crate) fn handle_inbound(app: &GuiApp, raw: &str) {
    let req: IpcRequest = match serde_json::from_str(raw) {
        Ok(r) => r,
        Err(e) => {
            app.append_log(format!("ipc: bad request: {e}"));
            return;
        },
    };
    let id = req.id;
    let cmd = req.cmd.clone();
    tracing::debug!(id, cmd = %cmd, "ipc dispatch");
    match command::dispatch(app, id, &req.cmd, &req.args) {
        CommandOutcome::Sync(result) => {
            let payload = command::reply_for_value(result);
            send_reply_payload(app, id, &payload);
        },
        CommandOutcome::Async => {},
    }
}
