/** @typedef {import("./types/BridgeError").BridgeError} BridgeError */
/** @typedef {import("./types/IpcReplyPayload").IpcReplyPayload} IpcReplyPayload */
/** @typedef {import("./types/IpcRequest").IpcRequest} IpcRequest */

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

export function invoke(cmd, args) {
  ensureBridge();
  return new Promise((resolve, reject) => {
    const id = nextId++;
    window.__bridge.pending.set(id, { resolve, reject });
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
  stateSnapshot:        ()                  => invoke("state.snapshot"),
  gatewaySet:           (url)               => invoke("gateway.set", { url }),
  gatewayProbe:         ()                  => invoke("gateway.probe"),
  login:                (token, gateway)    => invoke("login", { token, gateway }),
  logout:               ()                  => invoke("logout"),
  sync:                 ()                  => invoke("sync"),
  validate:             ()                  => invoke("validate"),
  marketplaceList:      ()                  => invoke("marketplace.list"),
  hostProbe:            (hostId)            => invoke("host.probe", { hostId }),
  hostProfileGenerate:  (hostId)            => invoke("host.profile.generate", { hostId }),
  hostProfileInstall:   (hostId, path)      => invoke("host.profile.install", { hostId, path }),
  hostProxyProbe:       ()                  => invoke("host.proxy.probe"),
  agentUninstall:       (hostId)            => invoke("agent.uninstall", { hostId }),
  agentOpenConfig:      (hostId)            => invoke("agent.openConfig", { hostId }),
  setupComplete:        ()                  => invoke("setup.complete"),
  openConfigFolder:     ()                  => invoke("openConfigFolder"),
  openLogFolder:        ()                  => invoke("openLogFolder"),
  diagnosticsExportBundle: ()               => invoke("diagnostics.exportBundle"),
  diagnosticsInfo:      ()                  => invoke("diagnostics.info"),
  cancel:               (scope)             => invoke("cancel", { scope: scope ?? "all" }),
  quit:                 ()                  => invoke("quit"),
};
