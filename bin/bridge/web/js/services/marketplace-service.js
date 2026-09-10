import { bridge } from "/assets/js/bridge.js";

export const MKT_KINDS = ["plugins", "skills", "rules", "hooks", "mcp", "agents", "artifacts"];

export function broadcastCount(listing) {
  if (!listing) { return; }
  let total = 0;
  for (const k of MKT_KINDS) { total += (listing[k] || []).length; }
  document.dispatchEvent(new CustomEvent("mkt:count", { detail: { total } }));
}

function fingerprint(snap) {
  const report = snap.last_sync_report;
  if (report && report.manifest_version) {
    return `manifest:${report.manifest_version}`;
  }
  return [
    snap.last_sync_summary,
    snap.skill_count, snap.plugin_count, snap.agent_count,
  ].join(" ");
}

function sessionKey(snap) {
  const identity = snap.verified_identity;
  return JSON.stringify([
    snap.gateway_url, identity?.tenant_id, identity?.user_id || identity?.email,
  ]);
}

export function createListingFetcher(onChange = () => {}) {
  let lastFingerprint = null;
  let lastSnapshot = null;
  let session = null;
  let syncing = false;
  let syncRefresh = false;
  let active = null;

  const self = {
    state: "idle",
    listing: null,
    error: null,
    reason: "signed-out",

    async maybeFetch(snap) {
      if (!snap) { return; }
      const key = sessionKey(snap);
      if (session !== null && session !== key) { reset(); }
      const transient = snap.verified_identity
        && ["verifying", "gateway-unreachable"].includes(snap.identity?.code);
      if (!snap.signed_in && !transient) {
        reset();
        return;
      }
      session = key;
      lastSnapshot = snap;
      syncRefresh ||= syncing && !snap.sync_in_flight;
      syncing = !!snap.sync_in_flight;
      if (!snap.signed_in) {
        if (!self.listing && !active) { set("idle", { reason: snap.identity.code }); }
        return;
      }
      const unchanged = self.listing !== null && fingerprint(snap) === lastFingerprint;
      if (unchanged && !syncRefresh && !active) { return; }
      const supersede = syncRefresh;
      syncRefresh = false;
      await run(snap, supersede);
    },

    async refresh() {
      if (lastSnapshot?.signed_in) { await run(lastSnapshot); }
    },
  };

  function set(state, extra) {
    self.state = state;
    Object.assign(self, extra || {});
    onChange();
  }

  function reset() {
    active = null;
    session = null;
    lastFingerprint = null;
    lastSnapshot = null;
    syncing = false;
    syncRefresh = false;
    const hadListing = self.listing !== null;
    set("idle", { listing: null, error: null, reason: "signed-out" });
    if (hadListing) { broadcastCount({}); }
  }

  function run(snap, supersede = false) {
    const marker = fingerprint(snap);
    if (active && active.marker === marker && !supersede) { return active.promise; }
    const request = { marker, promise: null };
    active = request;
    if (self.listing !== null) { set("ok", { error: null }); }
    else { set("loading", { error: null, reason: null }); }
    request.promise = fetchListing(snap, request);
    return request.promise;
  }

  async function fetchListing(snap, request) {
    try {
      const listing = await bridge.marketplaceList();
      if (active !== request) { return; }
      lastFingerprint = request.marker;
      set("ok", {
        listing,
        error: null,
        reason: snap.last_sync_summary ? "empty" : "never-synced",
      });
      broadcastCount(listing);
    } catch (e) {
      if (active !== request) { return; }
      set(self.listing !== null ? "ok" : "error", { error: (e && e.message) || String(e) });
    } finally {
      if (active === request) { active = null; }
    }
  }

  return self;
}
