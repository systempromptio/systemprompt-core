// One place that decides what a host's state *means* to the reader.
//
// The `chooseBadge` this replaces was correct but spoke in mechanism —
// "profile not installed", "awaiting first launch", "proxy down". The reader of
// this app is semi-technical: they need to know whether an agent is governed and
// working, and which single button fixes it if not. So the same inputs and the
// same precedence produce three user-facing states plus one reason line and one
// recommended action.
//
// Keeping this pure (no DOM, no bridge) means the row, the drawer and the add
// picker cannot drift into disagreeing about the same host.

import { t } from "/assets/js/i18n.js";
import { fmtRelative } from "/assets/js/utils/format.js";

export const APP_INSTALLED = "installed";
export const APP_NOT_INSTALLED = "not_installed";

/** Tri-state: an inconclusive probe is "unknown", which is NOT evidence of absence. */
export function appInstallState(host) {
  return (host && host.snapshot && host.snapshot.app_installed) || "unknown";
}

export function profileKind(host) {
  return (host && host.snapshot && host.snapshot.profile_state && host.snapshot.profile_state.kind) || "absent";
}

/**
 * Is this agent set up on this machine — i.e. does it belong in the Agents list
 * rather than behind "Add agent"? A host with no profile has no status worth
 * reporting; it is a thing you might add, which is a different question.
 */
export function isSetUp(host) {
  return profileKind(host) !== "absent";
}

/**
 * @returns {{state: "ok"|"attention"|"down"|"unknown", label: string,
 *            reason: string, action: {kind: string, label: string}|null}}
 */
export function hostStatus(host, snapshot) {
  const hs = (host && host.snapshot) || null;
  if (!hs) {
    return {
      state: "unknown",
      label: t("agent-state-checking") || "Checking…",
      reason: "",
      action: null,
    };
  }

  const kind = profileKind(host);
  const appState = appInstallState(host);
  const proxyState = ((snapshot && snapshot.local_proxy && snapshot.local_proxy.state) || "Unknown").toString();
  const modelsBlocked = !!host.models_checked && !host.compatible_models_available;
  const unconfigured = Array.isArray(host.unconfigured_providers) ? host.unconfigured_providers : [];

  const repair = { kind: "repair", label: t("agent-action-repair") || "Repair" };
  const verify = { kind: "verify", label: t("agent-action-verify") || "Verify" };
  const open = { kind: "open", label: t("host-action-open") || "Open" };

  // Precedence matches the old chooseBadge: the most specific, most actionable
  // fault wins, so the reader is never told "proxy down" when the real answer is
  // "the app isn't installed".
  if (appState === APP_NOT_INSTALLED) {
    return {
      state: "attention",
      label: t("agent-state-attention") || "Needs attention",
      reason: t("agent-reason-app-missing") || "The app is not installed on this computer",
      action: host.download_url
        ? { kind: "download", label: t("host-action-download") || "Download" }
        : null,
    };
  }
  if (kind === "stale") {
    return {
      state: "attention",
      label: t("agent-state-attention") || "Needs attention",
      reason: t("agent-reason-stale") || "Its settings are out of date — repair, then restart the app",
      action: repair,
    };
  }
  if (kind === "partial") {
    const missing = (host.snapshot.profile_state.missing_required || []).join(", ");
    return {
      state: "attention",
      label: t("agent-state-attention") || "Needs attention",
      reason: t("agent-reason-partial", { missing })
        || `Some settings are missing${missing ? ` (${missing})` : ""}`,
      action: repair,
    };
  }
  if (kind === "absent") {
    return {
      state: "attention",
      label: t("agent-state-not-set-up") || "Not set up",
      reason: t("agent-reason-absent") || "This agent is not routed through systemprompt yet",
      action: { kind: "add", label: t("agent-action-add") || "Add" },
    };
  }
  if (modelsBlocked) {
    return {
      state: "attention",
      label: t("agent-state-attention") || "Needs attention",
      reason: unconfigured.length
        ? (t("agent-reason-no-key", { providers: unconfigured.join(", ") })
            || `No usable model — add an API key for ${unconfigured.join(", ")}`)
        : (t("agent-reason-no-models") || "No model this agent can use is available"),
      action: null,
    };
  }
  if (proxyState === "Unconfigured") {
    return {
      state: "ok",
      label: t("agent-state-ready") || "Ready",
      reason: t("agent-reason-awaiting") || "Waiting for its first launch",
      action: open,
    };
  }
  if (proxyState === "Listening") {
    const probed = hs.probed_at_unix ? fmtRelative(hs.probed_at_unix) : "";
    return {
      state: "ok",
      label: t("agent-state-working") || "Working",
      reason: probed
        ? (t("agent-reason-governed-checked", { when: probed }) || `Governed · checked ${probed}`)
        : (t("agent-reason-governed") || "Governed"),
      action: open,
    };
  }
  return {
    state: "down",
    label: t("agent-state-down") || "Not working",
    reason: t("agent-reason-proxy-down") || "The local proxy is not responding",
    action: verify,
  };
}

/** Maps a status state onto the shared badge modifiers in badge.css. */
export function badgeSuffix(state) {
  if (state === "ok") { return "ok"; }
  if (state === "attention") { return "warn"; }
  if (state === "down") { return "err"; }
  return "muted";
}
