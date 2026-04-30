import { $, setBadge } from "./dom.js?t=__TOKEN__";

function classify(snap) {
  const cloudState = (snap.gateway_status && snap.gateway_status.state) || "unknown";
  if (cloudState === "probing" || cloudState === "unknown") {
    return { text: "checking…", cls: "sp-badge--muted" };
  } else if (cloudState === "unreachable") {
    return { text: "cloud unreachable", cls: "sp-badge--err" };
  } else {
    return classifyHosts(snap);
  }
}

function classifyHosts(snap) {
  const hosts = snap.host_apps || [];
  if (hosts.length === 0) {
    return { text: "no host apps", cls: "sp-badge--muted" };
  }
  const proxyState = (snap.local_proxy && snap.local_proxy.state || "Unknown").toString();
  const anyAbsent = hosts.some((h) => (h.snapshot && h.snapshot.profile_state && h.snapshot.profile_state.kind || "absent") === "absent");
  const anyPartial = hosts.some((h) => h.snapshot && h.snapshot.profile_state && h.snapshot.profile_state.kind === "partial");
  const allInstalled = hosts.every((h) => h.snapshot && h.snapshot.profile_state && h.snapshot.profile_state.kind === "installed");
  if (anyAbsent) {
    return { text: "profile not installed", cls: "sp-badge--warn" };
  } else if (anyPartial) {
    return { text: "profile partial", cls: "sp-badge--warn" };
  } else if (allInstalled && proxyState === "Unconfigured") {
    return { text: "awaiting first launch", cls: "sp-badge--warn" };
  } else if (allInstalled && proxyState === "Listening") {
    return { text: "healthy", cls: "sp-badge--ok" };
  } else if (allInstalled) {
    return { text: "local proxy down", cls: "sp-badge--err" };
  } else {
    return { text: "checking…", cls: "sp-badge--muted" };
  }
}

export function renderOverallBadge(snap) {
  const badge = $("overall-badge");
  const result = classify(snap);
  setBadge(badge, result.text, result.cls);
}
