/** @typedef {import("./types/BridgeError").BridgeError} BridgeError */
/** @typedef {import("./types/IpcReplyPayload").IpcReplyPayload} IpcReplyPayload */
/** @typedef {import("./types/IpcRequest").IpcRequest} IpcRequest */
/** @typedef {import("./types/StatePayload").StatePayload} StatePayload */

let nextId = 1;

// The bus is /assets/js/ipc-bootstrap.js, loaded before any module: by the
// webview as its initialization script and by index.html as a classic script.
// A page without it has no way to reach the bridge, so that is an error here,
// not a bus built on the spot that disagrees with the native side.
function ensureBridge() {
  const b = window.__bridge;
  if (!b || !b.__installed || typeof b.mount !== "number") {
    throw { scope: "internal", code: "internal", message: "ipc bootstrap missing" };
  }
  return b;
}

/**
 * @param {string} cmd
 * @param {object} [args]
 * @param {{ timeoutMs?: number }} [opts] a reply deadline. Off by default:
 *   sync and host installs legitimately run for minutes (UAC prompts included),
 *   so only read-style commands whose reply the UI cannot live without opt in.
 */
export function invoke(cmd, args, opts) {
  const bus = ensureBridge();
  return new Promise((resolve, reject) => {
    const id = nextId++;
    const timeoutMs = opts && opts.timeoutMs;
    // Why: a reply that never lands used to leave the promise pending for the
    // life of the window, and a pane waiting on it stayed on its skeleton with
    // nothing to click. A deadline turns that into an error the pane can retry.
    const timer = timeoutMs
      ? setTimeout(() => {
        if (!bus.pending.delete(id)) { return; }
        reject({ scope: "internal", code: "timeout", message: `${cmd} did not reply within ${Math.round(timeoutMs / 1000)}s` });
      }, timeoutMs)
      : null;
    const settle = (fn) => (v) => { if (timer) { clearTimeout(timer); } fn(v); };
    bus.pending.set(id, { resolve: settle(resolve), reject: settle(reject) });
    window.ipc.postMessage(JSON.stringify({ id, mount: bus.mount, cmd, args: args ?? {} }));
  });
}

export function subscribe(channel, cb) {
  const bus = ensureBridge();
  let set = bus.subs.get(channel);
  if (!set) { set = new Set(); bus.subs.set(channel, set); }
  set.add(cb);
  return () => set.delete(cb);
}

// Read-style commands answer from state the bridge already holds, or from a
// probe bounded on the native side; a reply that has not landed by then is a
// dead channel, which the pane must be told about rather than wait on.
const READ_TIMEOUT = { timeoutMs: 30_000 };

export const bridge = {
  invoke,
  subscribe,
  /** @returns {Promise<StatePayload>} */
  stateSnapshot:        ()                  => invoke("state.snapshot", {}, READ_TIMEOUT),
  gatewaySet:           (url)               => invoke("gateway.set", { url }),
  gatewayProbe:         ()                  => invoke("gateway.probe", {}, READ_TIMEOUT),
  login:                (token, gateway)    => invoke("login", { token, gateway }),
  signIn:               (gateway, keepSignedIn) => invoke("session.login", { gateway, keep_signed_in: !!keepSignedIn }),
  logout:               ()                  => invoke("logout"),
  systemPurge:          ()                  => invoke("system.purge"),
  systemDisconnect:     ()                  => invoke("system.disconnect"),
  deviceActionDismiss:  ()                  => invoke("device.action.dismiss"),
  removalGuidance:      ()                  => invoke("application.removalGuidance"),
  revealApplication:    ()                  => invoke("application.reveal"),
  sync:                 ()                  => invoke("sync"),
  validate:             ()                  => invoke("validate"),
  activityRecent:       (limit)             => invoke("activity.recent", { limit }, READ_TIMEOUT),
  marketplaceList:      ()                  => invoke("marketplace.list", {}, { timeoutMs: 30_000 }),
  profileFetch:         ()                  => invoke("profile.fetch", {}, READ_TIMEOUT),
  hostProbe:            (hostId)            => invoke("host.probe", { hostId }, READ_TIMEOUT),
  hostProfileGenerate:  (hostId)            => invoke("host.profile.generate", { hostId }, READ_TIMEOUT),
  hostProfileInstall:   (hostId, path)      => invoke("host.profile.install", { hostId, path }),
  hostProxyProbe:       ()                  => invoke("host.proxy.probe", {}, READ_TIMEOUT),
  mcpAuthProbe:         (serverId)          => invoke("mcp.auth.probe", serverId ? { serverId } : {}),
  hostModelFilterSet:   (hostId, protocols) => invoke("host.model-filter.set", { hostId, protocols }),
  agentUninstall:       (hostId)            => invoke("agent.uninstall", { hostId }),
  agentOpenConfig:      (hostId)            => invoke("agent.openConfig", { hostId }),
  agentOpen:            (hostId)            => invoke("agent.open", { hostId }),
  setupComplete:        ()                  => invoke("setup.complete", {}, READ_TIMEOUT),
  openConfigFolder:     ()                  => invoke("openConfigFolder"),
  openLogFolder:        ()                  => invoke("openLogFolder"),
  openExternalUrl:      (url)               => invoke("openExternalUrl", { url }),
  diagnosticsExportBundle: ()               => invoke("diagnostics.exportBundle"),
  proxyResetSecret:     ()                  => invoke("proxy.resetSecret"),
  configRepairDir:      ()                  => invoke("config.repairDir"),
  diagnosticsInfo:      ()                  => invoke("diagnostics.info", {}, READ_TIMEOUT),
  settingsGet:          ()                  => invoke("settings.get", {}, READ_TIMEOUT),
  updateCheck:          ()                  => invoke("update.check"),
  updateInstall:        ()                  => invoke("update.install"),
  updateRestart:        ()                  => invoke("update.restart"),
  cancel:               (scope)             => invoke("cancel", { scope: scope ?? "all" }),
  quit:                 ()                  => invoke("quit"),
};
