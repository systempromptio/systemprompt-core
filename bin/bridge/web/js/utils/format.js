export function fmtRelative(unix) {
  if (!unix) { return "never"; }
  const delta = Math.max(0, Math.floor(Date.now() / 1000) - unix);
  if (delta < 5) { return "just now"; }
  if (delta < 60) { return `${delta}s ago`; }
  if (delta < 3600) { return `${Math.floor(delta / 60)}m ago`; }
  return `${Math.floor(delta / 3600)}h ago`;
}

export function fmtDuration(seconds) {
  if (seconds == null) { return "—"; }
  const s = Math.max(0, Math.floor(seconds));
  if (s < 60) { return `${s}s`; }
  if (s < 3600) { return `${Math.floor(s / 60)}m ${s % 60}s`; }
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return `${h}h ${m}m`;
}

const BADGE_CLASS = {
  ok: "sp-badge--ok",
  warn: "sp-badge--warn",
  err: "sp-badge--err",
  probing: "sp-badge--warn",
  unknown: "sp-badge--muted",
};

export function publishSectionState(el, state, label) {
  const group = el.closest(".sp-status-group");
  if (!group) { return; }
  const badge = group.querySelector("[data-section-badge]");
  if (!badge) { return; }
  badge.classList.remove("sp-badge--ok", "sp-badge--warn", "sp-badge--err", "sp-badge--muted");
  badge.classList.add(BADGE_CLASS[state] || "sp-badge--muted");
  badge.textContent = label;
}
