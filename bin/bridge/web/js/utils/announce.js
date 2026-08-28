// One polite announcer for the whole app.
//
// The pattern this replaces was `aria-live="polite"` on a container that
// re-renders wholesale on every probe tick: the sync pill and the setup agent
// list both chattered, re-announcing unchanged text once a second. A live region
// belongs on something that changes only when there is news, and there is no
// such node in a component that repaints itself entirely -- so the region lives
// here instead and callers push to it deliberately.

let region = null;
let last = "";

function ensure() {
  if (region && region.isConnected) { return region; }
  region = document.createElement("div");
  region.className = "sp-visually-hidden";
  region.setAttribute("role", "status");
  region.setAttribute("aria-live", "polite");
  region.setAttribute("aria-atomic", "true");
  document.body.appendChild(region);
  return region;
}

/**
 * Announces `text` unless it is what was announced last — the de-duplication is
 * the point, because callers sit inside render paths that run on a timer.
 */
export function announce(text) {
  const msg = (text || "").trim();
  if (!msg || msg === last) { return; }
  last = msg;
  ensure().textContent = msg;
}

export function resetAnnouncer() {
  last = "";
}
