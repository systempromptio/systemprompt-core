import { bridge } from "/assets/js/bridge.js";

export const MKT_KINDS = ["plugins", "skills", "hooks", "mcp", "agents"];

export function broadcastCount(listing) {
  if (!listing) { return; }
  let total = 0;
  for (const k of MKT_KINDS) { total += (listing[k] || []).length; }
  document.dispatchEvent(new CustomEvent("mkt:count", { detail: { total } }));
}

export function createListingFetcher() {
  let inFlight = false;
  let lastSyncSummary = null;

  return {
    async maybeFetch(snap) {
      if (!snap || !snap.signed_in) { return null; }
      if (snap.last_sync_summary === lastSyncSummary && this._listing) { return null; }
      lastSyncSummary = snap.last_sync_summary;
      if (inFlight) { return null; }
      inFlight = true;
      try {
        const listing = await bridge.marketplaceList();
        this._listing = listing;
        broadcastCount(listing);
        return listing;
      } catch (e) {
        console.error("marketplace list failed", e);
        return null;
      } finally {
        inFlight = false;
      }
    },
    _listing: null,
  };
}
