#[expect(
    clippy::exit,
    reason = "GUI quit handler intentionally terminates the process"
)]
pub(crate) fn on_quit() {
    std::process::exit(0);
}
