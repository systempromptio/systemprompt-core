// The one IPC bus the page and the native side agree on.
//
// Loaded twice on purpose: the webview injects this file as its initialization
// script so the bus exists before any module runs, and index.html loads it as
// a classic script so the browser preview and the node tests get the same bus
// from the same bytes. The guard makes the second load a no-op.
//
// `mount` is minted once per page load. A request carries it and a reply names
// it, so a reply the native side produced for a page that has since been
// reloaded is recognised and dropped instead of settling a stranger's promise.
(function () {
  if (window.__bridge && window.__bridge.__installed) { return; }
  const mintMount = () => {
    const c = window.crypto;
    if (c && typeof c.getRandomValues === "function") {
      const words = new Uint32Array(2);
      c.getRandomValues(words);
      return ((words[0] & 0x1fffff) * 0x100000000) + words[1] || 1;
    }
    return Date.now();
  };
  const pending = new Map();
  const subs = new Map();
  const bus = {
    __installed: true,
    mount: mintMount(),
    pending,
    subs,
    reply(mount, id, payload) {
      if (mount !== bus.mount) { return; }
      const p = pending.get(id);
      if (!p) { return; }
      pending.delete(id);
      if (payload && payload.ok) { p.resolve(payload.value); }
      else { p.reject(payload && payload.error ? payload.error : { scope: "internal", code: "internal", message: "no payload" }); }
    },
    emit(channel, payload) {
      const set = subs.get(channel);
      if (!set) { return; }
      for (const cb of Array.from(set)) {
        try { cb(payload); } catch (e) { console.error("bridge subscriber threw", e); }
      }
    },
  };
  window.__bridge = bus;
})();
