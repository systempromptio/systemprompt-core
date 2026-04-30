use std::path::PathBuf;

use systemprompt_bridge::config::paths::{
    Scope, org_plugins_effective, org_plugins_system, org_plugins_user,
};

#[test]
fn both_scopes_resolvable() {
    assert!(
        org_plugins_system().is_some(),
        "system scope should resolve on every supported OS"
    );
    assert!(
        org_plugins_user().is_some(),
        "user scope should resolve when HOME/XDG is set"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn macos_system_path() {
    assert_eq!(
        org_plugins_system().unwrap(),
        PathBuf::from("/Library/Application Support/Claude/org-plugins")
    );
}

#[cfg(target_os = "macos")]
#[test]
fn macos_effective_is_always_system_scope() {
    let loc = org_plugins_effective().expect("system path resolves on macOS");
    assert_eq!(loc.scope, Scope::System);
    assert_eq!(
        loc.path,
        PathBuf::from("/Library/Application Support/Claude/org-plugins")
    );
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn linux_user_path_respects_xdg() {
    let _guard = env_lock();
    let prev = std::env::var_os("XDG_DATA_HOME");
    set_xdg("/tmp/xdg-test");
    let p = org_plugins_user().unwrap();
    match prev {
        Some(v) => set_xdg_os(&v),
        None => clear_xdg(),
    }
    assert_eq!(p, PathBuf::from("/tmp/xdg-test/Claude/org-plugins"));
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn set_xdg(v: &str) {
    unsafe { std::env::set_var("XDG_DATA_HOME", v) }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn set_xdg_os(v: &std::ffi::OsStr) {
    unsafe { std::env::set_var("XDG_DATA_HOME", v) }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn clear_xdg() {
    unsafe { std::env::remove_var("XDG_DATA_HOME") }
}
