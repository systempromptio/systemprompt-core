import { announce } from "/assets/js/utils/announce.js";

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

/**
 * The user-facing form. `fmtDuration` is the compact one for a status pill;
 * this is what goes in a sentence, because "expires in ~540s" names a unit
 * nobody thinks in.
 */
export function fmtDurationLong(seconds) {
  if (seconds == null) { return "—"; }
  const s = Math.max(0, Math.floor(seconds));
  if (s < 60) { return `less than a minute`; }
  const m = Math.round(s / 60);
  if (m < 60) { return `${m} minute${m === 1 ? "" : "s"}`; }
  const h = Math.round(s / 3600);
  if (h < 24) { return `${h} hour${h === 1 ? "" : "s"}`; }
  const d = Math.round(s / 86400);
  return `${d} day${d === 1 ? "" : "s"}`;
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
  if (badge.textContent === label) { return; }
  badge.textContent = label;
  // These four badges flip from "checking…" to a real state with nothing to say
  // so for anyone not looking at them. One polite announcer rather than four
  // live regions, so a page of settling groups reads as a few lines, not four
  // simultaneous interruptions.
  const heading = group.querySelector("h2");
  announce(heading ? `${heading.textContent.trim()}: ${label}` : label);
}
