import { bridge } from "/assets/js/bridge.js";

export const MKT_KINDS = ["plugins", "skills", "hooks", "mcp", "agents", "artifacts"];

export function broadcastCount(listing) {
  if (!listing) { return; }
  let total = 0;
  for (const k of MKT_KINDS) { total += (listing[k] || []).length; }
  document.dispatchEvent(new CustomEvent("mkt:count", { detail: { total } }));
}

// Why: `manifest_version` is the manifest's own identity, so it changes exactly
// when the listing does. It only exists once a sync has completed in this
// process — `AppState::reload()` re-derives the flattened string from the
// on-disk sentinel, which does not carry the structured report — so the counts
// stay as the fallback arm rather than being replaced by it.
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

/**
 * The Library's listing, with its state said out loud.
 *
 * The old fetcher answered `null` for four different situations — not signed
 * in, unchanged, already in flight, and failed — so the pane rendered all four
 * as "nothing here". Callers now read `state` and can tell them apart.
 *
 * @param {() => void} onChange called on every transition, so a loading state
 * is painted before the request is awaited rather than after it resolves.
 */
export function createListingFetcher(onChange = () => {}) {
  let lastFingerprint = null;
  let lastSnapshot = null;
  let syncing = false;

  const self = {
    state: "idle",
    listing: null,
    error: null,
    reason: "signed-out",

    async maybeFetch(snap) {
      if (!snap) { return; }
      if (!snap.signed_in) {
        set("idle", { reason: "signed-out", listing: null });
        return;
      }
      if (self.state === "loading") { return; }
      // A sync that produced an identical summary string still changes what is
      // on disk, so a run finishing always refetches rather than trusting the
      // flattened one-line summary to have moved.
      const syncJustFinished = syncing && !snap.sync_in_flight;
      syncing = !!snap.sync_in_flight;
      const unchanged = self.state === "ok" && fingerprint(snap) === lastFingerprint;
      if (unchanged && !syncJustFinished) { return; }
      await run(snap);
    },

    async refresh() {
      if (lastSnapshot) { await run(lastSnapshot); }
    },
  };

  function set(state, extra) {
    self.state = state;
    Object.assign(self, extra || {});
    onChange();
  }

  async function run(snap) {
    lastSnapshot = snap;
    set("loading", { error: null, reason: null });
    try {
      const listing = await bridge.marketplaceList();
      // The marker only advances on a successful fetch. Advancing it up front
      // was why a dropped or failed request never retried.
      lastFingerprint = fingerprint(snap);
      set("ok", {
        listing,
        error: null,
        reason: snap.last_sync_summary ? "empty" : "never-synced",
      });
      broadcastCount(listing);
    } catch (e) {
      set("error", { error: (e && e.message) || String(e) });
    }
  }

  return self;
}
