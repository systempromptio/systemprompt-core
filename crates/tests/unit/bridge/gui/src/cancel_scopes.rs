use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::gui::state::{AppState, CancelScope};

fn state() -> std::sync::Arc<AppState> {
    let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
    AppState::new_loaded(ctx)
}

// Why: purge and sign-out rely on this to stop an in-flight browser sign-in.
// If they were "fixed" with `clear_cancel` instead, the login task would keep
// running, its loopback listener would keep accepting, and the callback would
// write the credential back after the purge deleted it.
#[test]
fn cancel_scope_stops_an_in_flight_login() {
    let state = state();
    let token = state.install_cancel(CancelScope::Login);
    assert!(!token.is_cancelled(), "a fresh login token is live");

    assert!(
        state.cancel_scope(CancelScope::Login),
        "cancelling reports that there was a login to cancel"
    );
    assert!(
        token.is_cancelled(),
        "the task holding this token observes the cancellation"
    );
    assert!(!state.has_cancel(CancelScope::Login));
}

#[test]
fn clear_cancel_does_not_stop_the_task() {
    let state = state();
    let token = state.install_cancel(CancelScope::Login);

    state.clear_cancel(CancelScope::Login);

    assert!(
        !token.is_cancelled(),
        "clear_cancel only forgets the token; it is not a way to stop a login"
    );
}

#[test]
fn cancelling_login_leaves_an_unrelated_sync_running() {
    let state = state();
    let login = state.install_cancel(CancelScope::Login);
    let sync = state.install_cancel(CancelScope::Sync);

    state.cancel_scope(CancelScope::Login);

    assert!(login.is_cancelled());
    assert!(
        !sync.is_cancelled(),
        "purge and sign-out must not take an in-flight sync down with the login"
    );
}
