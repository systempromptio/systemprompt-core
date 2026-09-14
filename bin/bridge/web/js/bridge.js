/** @typedef {import("./types/BridgeError").BridgeError} BridgeError */
/** @typedef {import("./types/IpcReplyPayload").IpcReplyPayload} IpcReplyPayload */
/** @typedef {import("./types/IpcRequest").IpcRequest} IpcRequest */
/** @typedef {import("./types/StatePayload").StatePayload} StatePayload */

let nextId = 1;

function ensureBridge() {
  const w = window;
  if (!w.__bridge) {
    w.__bridge = {
      __installed: true,
      pending: new Map(),
      subs: new Map(),
      reply(id, payload) {
        const p = w.__bridge.pending.get(id);
        if (!p) { return; }
        w.__bridge.pending.delete(id);
        if (payload && payload.ok) { p.resolve(payload.value); }
        else { p.reject(payload && payload.error ? payload.error : { scope: "internal", code: "internal", message: "no payload" }); }
      },
      emit(channel, payload) {
        const set = w.__bridge.subs.get(channel);
        if (!set) { return; }
        for (const cb of Array.from(set)) {
          try { cb(payload); } catch (e) { console.error("bridge subscriber threw", e); }
        }
      },
    };
    return;
  }
  if (!w.__bridge.pending) { w.__bridge.pending = new Map(); }
  if (!w.__bridge.subs) { w.__bridge.subs = new Map(); }
}

/**
 * @param {string} cmd
 * @param {object} [args]
 * @param {{ timeoutMs?: number }} [opts] a reply deadline. Off by default:
 *   sync and host installs legitimately run for minutes (UAC prompts included),
 *   so only read-style commands whose reply the UI cannot live without opt in.
 */
export function invoke(cmd, args, opts) {
  ensureBridge();
  return new Promise((resolve, reject) => {
    const id = nextId++;
    const timeoutMs = opts && opts.timeoutMs;
    // Why: a reply that never lands used to leave the promise pending for the
    // life of the window, and a pane waiting on it stayed on its skeleton with
    // nothing to click. A deadline turns that into an error the pane can retry.
    const timer = timeoutMs
      ? setTimeout(() => {
        if (!window.__bridge.pending.delete(id)) { return; }
        reject({ scope: "internal", code: "timeout", message: `${cmd} did not reply within ${Math.round(timeoutMs / 1000)}s` });
      }, timeoutMs)
      : null;
    const settle = (fn) => (v) => { if (timer) { clearTimeout(timer); } fn(v); };
    window.__bridge.pending.set(id, { resolve: settle(resolve), reject: settle(reject) });
    window.ipc.postMessage(JSON.stringify({ id, cmd, args: args ?? {} }));
  });
}

export function subscribe(channel, cb) {
  ensureBridge();
  let set = window.__bridge.subs.get(channel);
  if (!set) { set = new Set(); window.__bridge.subs.set(channel, set); }
  set.add(cb);
  return () => set.delete(cb);
}

export const bridge = {
  invoke,
  subscribe,
  /** @returns {Promise<StatePayload>} */
  stateSnapshot:        ()                  => invoke("state.snapshot"),
  gatewaySet:           (url)               => invoke("gateway.set", { url }),
  gatewayProbe:         ()                  => invoke("gateway.probe"),
  login:                (token, gateway)    => invoke("login", { token, gateway }),
  signIn:               (gateway, keepSignedIn) => invoke("session.login", { gateway, keep_signed_in: !!keepSignedIn }),
  logout:               ()                  => invoke("logout"),
  systemPurge:          ()                  => invoke("system.purge"),
  systemDisconnect:     ()                  => invoke("system.disconnect"),
  deviceActionOpen:     (action)            => invoke("device.action.open", { action }),
  deviceActionDismiss:  ()                  => invoke("device.action.dismiss"),
  removalGuidance:      ()                  => invoke("application.removalGuidance"),
  revealApplication:    ()                  => invoke("application.reveal"),
  sync:                 ()                  => invoke("sync"),
  validate:             ()                  => invoke("validate"),
  activityRecent:       (limit)             => invoke("activity.recent", { limit }),
  marketplaceList:      ()                  => invoke("marketplace.list", {}, { timeoutMs: 30_000 }),
  profileFetch:         ()                  => invoke("profile.fetch"),
  hostProbe:            (hostId)            => invoke("host.probe", { hostId }),
  hostProfileGenerate:  (hostId)            => invoke("host.profile.generate", { hostId }),
  hostProfileInstall:   (hostId, path)      => invoke("host.profile.install", { hostId, path }),
  hostProxyProbe:       ()                  => invoke("host.proxy.probe"),
  mcpAuthProbe:         (serverId)          => invoke("mcp.auth.probe", serverId ? { serverId } : {}),
  hostModelFilterSet:   (hostId, protocols) => invoke("host.model-filter.set", { hostId, protocols }),
  agentUninstall:       (hostId)            => invoke("agent.uninstall", { hostId }),
  agentOpenConfig:      (hostId)            => invoke("agent.openConfig", { hostId }),
  agentOpen:            (hostId)            => invoke("agent.open", { hostId }),
  setupComplete:        ()                  => invoke("setup.complete"),
  openConfigFolder:     ()                  => invoke("openConfigFolder"),
  openLogFolder:        ()                  => invoke("openLogFolder"),
  openExternalUrl:      (url)               => invoke("openExternalUrl", { url }),
  diagnosticsExportBundle: ()               => invoke("diagnostics.exportBundle"),
  proxyResetSecret:     ()                  => invoke("proxy.resetSecret"),
  diagnosticsInfo:      ()                  => invoke("diagnostics.info"),
  settingsGet:          ()                  => invoke("settings.get"),
  updateCheck:          ()                  => invoke("update.check"),
  updateInstall:        ()                  => invoke("update.install"),
  updateRestart:        ()                  => invoke("update.restart"),
  cancel:               (scope)             => invoke("cancel", { scope: scope ?? "all" }),
  quit:                 ()                  => invoke("quit"),
};
