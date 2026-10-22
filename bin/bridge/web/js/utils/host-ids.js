// The bridge host ids, mirrored from `HostKind::ALL` in
// crates/shared/models/src/bridge/host.rs. A Rust test
// (crates/tests/unit/bridge/verdicts/src/host_ids_js.rs) fails when the two
// lists differ, so this is the only place the web tree spells a host id.

export const HOST_ID = Object.freeze({
  claudeCode: "claude-code",
  claudeDesktop: "claude-desktop",
  codex: "codex",
  codexCli: "codex-cli",
  opencode: "opencode",
  opencodeCli: "opencode-cli",
  hermes: "hermes",
});

export const HOST_IDS = Object.freeze(Object.values(HOST_ID));
