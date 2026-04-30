import { t } from "/assets/js/i18n.js";

export function probeView(snap) {
  const status = (snap && snap.gateway_status) || { state: "unknown" };
  if (status.state === "reachable") {
    return { dot: "sp-dot--ok", muted: false, text: t("setup-gateway-reachable", { latency: status.latency_ms }) || `reachable · ${status.latency_ms}ms` };
  }
  if (status.state === "probing") {
    return { dot: "sp-dot--probing", muted: true, text: t("setup-gateway-probing") || "probing…" };
  }
  if (status.state === "unreachable") {
    return { dot: "sp-dot--err", muted: false, text: t("setup-gateway-unreachable", { reason: status.reason || "unknown" }) || `unreachable · ${status.reason || "unknown"}` };
  }
  const empty = !(snap && snap.gateway_url);
  return { dot: "sp-dot--unknown", muted: true, text: empty ? (t("setup-gateway-empty") || "enter a URL to probe…") : (t("setup-gateway-not-probed") || "not probed yet") };
}

export function probeErrorMessage(snap) {
  if (!snap) { return ""; }
  const status = snap.gateway_status || { state: "unknown" };
  const verified = snap.verified_identity && snap.verified_identity.user_id;
  if (status.state === "reachable" && snap.pat_present && !verified) {
    return "Token rejected by gateway. Issue a fresh PAT and try again.";
  }
  if (status.state === "unreachable" && snap.pat_present) {
    return `Gateway unreachable: ${status.reason || "unknown error"}`;
  }
  return "";
}

export function isPendingResolved(snap, pendingSinceMs) {
  if (!snap) { return false; }
  const probeState = (snap.gateway_status && snap.gateway_status.state) || "unknown";
  const configured = probeState === "reachable" && snap.verified_identity && snap.verified_identity.user_id;
  const elapsed = pendingSinceMs > 0 ? (Date.now() - pendingSinceMs) : 0;
  return configured || probeState === "unreachable" || elapsed > 15000;
}

export function patLinkFor(gateway) {
  const gw = (gateway || "").trim().replace(/\/+$/, "");
  if (gw) { return `${gw}/admin/login`; }
  return "#";
}
