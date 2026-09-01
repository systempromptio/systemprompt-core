// Renders the verdicts the bridge computes in Rust.
//
// This file deliberately contains no logic about what a state *means*. That
// derivation used to live here, in the Agents list, in the Status summary card,
// in the overall badge, in the MCP pane and in the Home card — copies that
// disagreed with each other about the same snapshot. It now happens once, in
// Rust beside each enum (`src/verdict.rs`), and arrives as `{ tone, code }`.
//
// Every code is a kebab string that is also the FTL key suffix, so each
// function below is a lookup. If you find yourself adding a conditional on a
// state's name here, the logic belongs in Rust — and
// `scripts/lint-bridge-verdicts.sh` will refuse it.

import { t } from "/assets/js/i18n.js";
import { fmtRelative } from "/assets/js/utils/format.js";

const TONE_RANK = { ok: 0, unknown: 1, probing: 2, warn: 3, err: 4 };

/** `sp-dot--*` modifier for a tone. */
export function toneDot(tone) {
  return `sp-dot--${TONE_RANK[tone] === undefined ? "unknown" : tone}`;
}

/** `sp-badge--*` suffix for a tone. */
export function toneBadge(tone) {
  if (tone === "ok" || tone === "warn" || tone === "err") { return tone; }
  return "muted";
}

/** The section badge word for a tone. */
export function toneSection(tone) {
  return t(`tone-section-${TONE_RANK[tone] === undefined ? "unknown" : tone}`) || "";
}

/** The worse of two tones; folding is arithmetic on tones, not a derivation. */
export function worstTone(a, b) {
  return (TONE_RANK[a] ?? 1) >= (TONE_RANK[b] ?? 1) ? a : b;
}

/** @returns {object} the verdict, or a Checking placeholder if none arrived. */
export function verdictOf(host) {
  return (host && host.verdict) || { state: "checking", tone: "unknown", reason: { code: "never-probed" }, action: null, is_set_up: false, is_running: false };
}

export function isSetUp(host) {
  return verdictOf(host).is_set_up === true;
}

export function stateLabel(state) {
  return t(`agent-state-${state}`) || "";
}

/**
 * The reason line. Fluent arguments come straight off the reason payload; the
 * only thing computed here is relative time, which is genuinely client-side.
 */
export function reasonLabel(reason) {
  if (!reason || !reason.code) { return ""; }
  if (reason.code === "governed" && reason.when_unix) {
    return t("agent-reason-governed-checked", { when: fmtRelative(reason.when_unix) })
      || t("agent-reason-governed") || "";
  }
  return t(`agent-reason-${reason.code}`, reason) || "";
}

export function actionLabel(action) {
  return action && action.code ? (t(`agent-action-${action.code}`) || "") : "";
}

/** Maps a verdict or fleet state onto the shared badge/dot modifiers. */
export function badgeSuffix(state) {
  switch (state) {
    case "working": case "ready": case "ok": return "ok";
    case "attention": case "not-set-up": case "warn": return "warn";
    case "down": case "err": return "err";
    default: return "muted";
  }
}

export function dotClass(state) {
  return `sp-dot--${badgeSuffix(state) === "muted" ? "unknown" : badgeSuffix(state)}`;
}

/** The section badge word for a fleet state. */
export function sectionLabel(state) {
  return toneSection(state);
}

export function fleetHeadline(headline) {
  return t(`agents-fleet-${headline}`) || "";
}

export const APP_INSTALLED = "installed";
export const APP_NOT_INSTALLED = "not_installed";

/** The app-install verdict code. Tri-state: `unknown` is NOT evidence of absence. */
export function appInstallState(host) {
  return (host && host.health && host.health.app && host.health.app.code) || "unknown";
}

/**
 * The verdict in the shape the row and drawer render.
 *
 * Presentation only — every field is read straight off `host.verdict`. It takes
 * no snapshot argument, because there is nothing left here to derive from one.
 */
export function statusOf(host) {
  const v = verdictOf(host);
  return {
    state: v.state,
    tone: v.tone,
    label: stateLabel(v.state),
    reason: reasonLabel(v.reason),
    action: v.action ? { code: v.action.code, label: actionLabel(v.action) } : null,
  };
}

/** The managed profile is present and complete (not partial, not stale). */
export function isInstalled(host) {
  return verdictOf(host).is_installed === true;
}
