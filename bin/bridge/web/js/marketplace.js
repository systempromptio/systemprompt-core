import { $ } from "./dom.js?t=__TOKEN__";
import { apiGet } from "./api.js?t=__TOKEN__";
import { mktState } from "./marketplace/state.js?t=__TOKEN__";
import { renderList, syncCategorySelection, updateCounts } from "./marketplace/list.js?t=__TOKEN__";
import { renderDetail, copyToClipboard as detailCopy } from "./marketplace/detail.js?t=__TOKEN__";

export function renderMarketplace() {
  if (mktState.data) {
    updateCounts();
    syncCategorySelection();
    renderList();
    renderDetail();
  }
}

async function fetchMarketplace() {
  if (mktState.inFlight) {
    return;
  }
  mktState.inFlight = true;
  try {
    mktState.data = await apiGet("/api/marketplace");
    renderMarketplace();
  } catch (e) {
    console.error("marketplace fetch failed", e);
  } finally {
    mktState.inFlight = false;
  }
}

export function maybeRefreshMarketplace(snap) {
  if (snap.signed_in) {
    if (snap.last_sync_summary !== mktState.lastSyncSummary || !mktState.data) {
      mktState.lastSyncSummary = snap.last_sync_summary;
      fetchMarketplace();
    }
  }
}

export function selectMarketplaceKind(kind) {
  mktState.kind = kind;
  mktState.selectedId = null;
  renderMarketplace();
}

export function selectMarketplaceItem(id) {
  mktState.selectedId = id;
  renderMarketplace();
}

export function setMarketplaceSearch(value) {
  mktState.search = value;
  const search = $("mkt-search");
  if (search && search.value !== value) {
    search.value = value;
  }
  renderMarketplace();
}

export function copyToClipboard(button, value) {
  return detailCopy(button, value);
}

export function renderMarketplaceBadge(snap) {
  const badge = $("marketplace-status");
  if (badge) {
    badge.classList.remove("sp-badge--muted", "sp-badge--ok", "sp-badge--warn", "sp-badge--err");
    if (!snap.signed_in) {
      badge.textContent = "sign-in required";
      badge.classList.add("sp-badge--warn");
    } else if (snap.sync_in_flight) {
      badge.textContent = "syncing";
      badge.classList.add("sp-badge--warn");
    } else if (snap.last_sync_summary) {
      badge.textContent = "synced";
      badge.classList.add("sp-badge--ok");
    } else {
      badge.textContent = "never synced";
      badge.classList.add("sp-badge--muted");
    }
  }
}

export function initMarketplace() {
  const search = $("mkt-search");
  if (search) {
    search.addEventListener("input", (e) => {
      mktState.search = e.target.value || "";
      renderMarketplace();
    });
  }
}
