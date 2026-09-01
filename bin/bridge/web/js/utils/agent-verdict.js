// Renders the agent-health verdict the bridge computes in Rust.
//
// This file deliberately contains no logic about what a host's state *means*.
// That derivation used to live here, in the Agents list, in the Status summary
// card and in the overall badge — four copies, four `|| "absent"` fallbacks,
// and no two of them agreeing about a host whose proxy was down. It now happens
// once, in `integration/agent_health.rs`, and arrives as `host.verdict`.
//
// Every enum serialises to a kebab code that is also the FTL key suffix, so
// each function below is a lookup. If you find yourself adding a conditional on
// `profile_state` here, the logic belongs in Rust.

import { t } from "/assets/js/i18n.js";
import { fmtRelative } from "/assets/js/utils/format.js";

/** @returns {object} the verdict, or a Checking placeholder if none arrived. */
export function verdictOf(host) {
  return (host && host.verdict) || { state: "checking", reason: { code: "never-probed" }, action: null, is_set_up: false, is_running: false };
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
  switch (state) {
    case "ok": return "healthy";
    case "warn": return "attention";
    case "err": return "down";
    default: return "unknown";
  }
}

export function fleetHeadline(headline) {
  return t(`agents-fleet-${headline}`) || "";
}

export const APP_INSTALLED = "installed";
export const APP_NOT_INSTALLED = "not_installed";

/** Raw snapshot field. Tri-state: `unknown` is NOT evidence of absence. */
export function appInstallState(host) {
  return (host && host.snapshot && host.snapshot.app_installed) || "unknown";
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
    label: stateLabel(v.state),
    reason: reasonLabel(v.reason),
    action: v.action ? { kind: v.action.code, label: actionLabel(v.action) } : null,
  };
}

/** The managed profile is present and complete (not partial, not stale). */
export function isInstalled(host) {
  return verdictOf(host).is_installed === true;
}
