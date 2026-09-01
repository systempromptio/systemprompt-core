import { t } from "/assets/js/i18n.js";
import { toneDot } from "/assets/js/utils/verdict.js";

export function reachabilityView(gateway) {
  const tone = gateway.tone || "unknown";
  const reachable = tone === "ok";
  return {
    tone,
    dot: toneDot(tone),
    value: reachable ? String(gateway.latency_ms ?? "?") : (tone === "probing" ? "…" : "—"),
    unit: reachable ? "ms" : "",
    label: t(`gateway-state-${gateway.code || "unknown"}`, { latency: gateway.latency_ms ?? "?", reason: gateway.reason || "unknown error" }) || "",
    reason: tone === "err" ? (gateway.reason || "unknown error") : "",
  };
}

export function identityView(snap) {
  const identity = snap.identity || { tone: "unknown", code: "signed-out" };
  const id = snap.verified_identity || {};
  const who = id.email || id.user_id || "";
  return {
    tone: identity.tone,
    dot: toneDot(identity.tone),
    value: identity.tone === "ok" ? who : (identity.tone === "probing" ? "…" : "—"),
    label: t(`identity-${identity.code}`) || "",
    muted: identity.tone !== "ok",
  };
}

export function cloudTokenSummary(snap) {
  if (snap.cached_token) { return `JWT · ${snap.cached_token.ttl_seconds}s`; }
  return snap.pat_present ? "PAT stored" : "no token";
}

export function cloudTokenDetail(snap) {
  if (snap.cached_token) {
    return `JWT · ${snap.cached_token.length} bytes · ttl ${snap.cached_token.ttl_seconds}s`;
  }
  return snap.pat_present ? "PAT stored — JWT will refresh on next probe" : "none";
}

export function canLogout(snap) {
  const id = snap.verified_identity || {};
  return Boolean(id.email || id.user_id || snap.pat_present);
}
