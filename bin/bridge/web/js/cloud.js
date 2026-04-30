import { $, setDot, fmtRelative } from "./dom.js?t=__TOKEN__";

function applyServerStatus(status) {
  const dot = $("server-dot");
  const text = $("server-state-text");
  let cls = "sp-dot--unknown";
  let label = "unknown";
  if (status.state === "reachable") {
    cls = "sp-dot--ok";
    label = `reachable · ${status.latency_ms}ms`;
  } else if (status.state === "probing") {
    cls = "sp-dot--probing";
    label = "probing…";
  } else if (status.state === "unreachable") {
    cls = "sp-dot--err";
    label = `unreachable · ${status.reason || "unknown error"}`;
  } else {
    cls = "sp-dot--unknown";
    label = "unknown";
  }
  setDot(dot, cls);
  if (text) {
    text.textContent = label;
  }
}

function applyIdentity(snap, reachable) {
  const id = snap.verified_identity;
  const idEl = $("identity");
  const idDot = $("identity-dot");
  if (!idEl || !idDot) {
    return;
  }
  if (!reachable) {
    setDot(idDot, "sp-dot--unknown");
    idEl.textContent = "(gateway unreachable)";
    idEl.classList.add("sp-u-muted");
  } else if (id && (id.email || id.user_id)) {
    setDot(idDot, "sp-dot--ok");
    idEl.textContent = id.email || id.user_id;
    idEl.classList.remove("sp-u-muted");
  } else if (snap.pat_present) {
    setDot(idDot, "sp-dot--probing");
    idEl.textContent = "(verifying credentials…)";
    idEl.classList.add("sp-u-muted");
  } else {
    setDot(idDot, "sp-dot--warn");
    idEl.textContent = "(not signed in)";
    idEl.classList.add("sp-u-muted");
  }
  $("identity-user").textContent = (id && id.user_id) || "—";
  $("identity-tenant").textContent = (id && id.tenant_id) || "—";
}

export function renderCloud(snap) {
  const status = snap.gateway_status || { state: "unknown" };
  applyServerStatus(status);

  const endpoint = $("server-endpoint");
  if (endpoint) {
    endpoint.textContent = snap.gateway_url || "—";
    endpoint.classList.toggle("sp-u-muted", !snap.gateway_url);
  }
  const probe = $("server-probe");
  if (probe) {
    probe.textContent = fmtRelative(snap.last_probe_at_unix);
    probe.classList.toggle("sp-u-muted", !snap.last_probe_at_unix);
  }

  applyIdentity(snap, status.state === "reachable");

  const tokenState = snap.cached_token
    ? `cached JWT • ${snap.cached_token.length} bytes • ttl ${snap.cached_token.ttl_seconds}s`
    : (snap.pat_present ? "PAT stored — JWT will refresh on next probe" : "no token");
  const tokenEl = $("token-state");
  if (tokenEl) {
    tokenEl.textContent = tokenState;
  }
}
