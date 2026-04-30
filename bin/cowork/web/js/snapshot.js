import { $, setDot, setBadge, fmtRelative } from "./dom.js?t=__TOKEN__";
import { post } from "./api.js?t=__TOKEN__";
import { append } from "./activity.js?t=__TOKEN__";
import { syncRailIndicator } from "./tabs.js?t=__TOKEN__";
import { setSetupError, updateSetupPatLink, setLastSavedGateway } from "./setup.js?t=__TOKEN__";

const hostCards = new Map();

function renderCloud(snap) {
  const status = snap.gateway_status || { state: "unknown" };
  const dot = $("server-dot");
  const text = $("server-state-text");
  let label = "unknown";
  let cls = "dot-unknown";
  switch (status.state) {
    case "reachable":   cls = "dot-ok";      label = `reachable · ${status.latency_ms}ms`; break;
    case "probing":     cls = "dot-probing"; label = "probing…"; break;
    case "unreachable": cls = "dot-err";     label = `unreachable · ${status.reason || "unknown error"}`; break;
    default:            cls = "dot-unknown"; label = "unknown";
  }
  setDot(dot, cls);
  text.textContent = label;
  $("server-endpoint").textContent = snap.gateway_url || "—";
  $("server-endpoint").classList.toggle("muted", !snap.gateway_url);
  $("server-probe").textContent = fmtRelative(snap.last_probe_at_unix);
  $("server-probe").classList.toggle("muted", !snap.last_probe_at_unix);

  const reachable = status.state === "reachable";
  const id = snap.verified_identity;
  const idEl = $("identity");
  const idDot = $("identity-dot");
  if (!reachable) {
    setDot(idDot, "dot-unknown");
    idEl.textContent = "(gateway unreachable)";
    idEl.classList.add("muted");
  } else if (id && (id.email || id.user_id)) {
    setDot(idDot, "dot-ok");
    idEl.textContent = id.email || id.user_id;
    idEl.classList.remove("muted");
  } else if (snap.pat_present) {
    setDot(idDot, "dot-probing");
    idEl.textContent = "(verifying credentials…)";
    idEl.classList.add("muted");
  } else {
    setDot(idDot, "dot-warn");
    idEl.textContent = "(not signed in)";
    idEl.classList.add("muted");
  }
  $("identity-user").textContent = id && id.user_id ? id.user_id : "—";
  $("identity-tenant").textContent = id && id.tenant_id ? id.tenant_id : "—";

  const tokenState = snap.cached_token
    ? `cached JWT • ${snap.cached_token.length} bytes • ttl ${snap.cached_token.ttl_seconds}s`
    : (snap.pat_present ? "PAT stored — JWT will refresh on next probe" : "no token");
  $("token-state").textContent = tokenState;
}

function collectInferenceModels(snap) {
  const seen = new Set();
  const out = [];
  for (const host of (snap.host_apps || [])) {
    const raw = host.snapshot?.profile_keys?.inferenceModels;
    if (!raw) continue;
    for (const m of raw.split(",")) {
      const t = m.trim();
      if (t && !seen.has(t)) { seen.add(t); out.push(t); }
    }
  }
  return out;
}

function renderProxy(snap) {
  const proxy = snap.local_proxy || { state: "Unknown" };
  const state = (proxy.state || "Unknown").toString();
  const dot = $("proxy-dot");
  const text = $("proxy-text");
  let cls = "dot-unknown";
  let label = state;
  switch (state) {
    case "Listening":    cls = "dot-ok";   label = `listening · ${proxy.latency_ms ?? "?"}ms`; break;
    case "Refused":      cls = "dot-err";  label = "connection refused"; break;
    case "Timeout":      cls = "dot-err";  label = "timed out"; break;
    case "HttpError":    cls = "dot-err";  label = `error: ${proxy.error || "unknown"}`; break;
    case "Unconfigured": cls = "dot-warn"; label = "awaiting first host-app probe"; break;
    default:             cls = "dot-unknown"; label = "checking…";
  }
  setDot(dot, cls);
  text.textContent = label;
  $("proxy-detail").textContent = proxy.url || "(no proxy URL configured yet)";
  $("proxy-detail").classList.toggle("muted", !proxy.url);

  const models = collectInferenceModels(snap);
  const epDot = $("endpoints-dot");
  const epText = $("endpoints-text");
  if (models.length === 0) {
    setDot(epDot, "dot-unknown");
    epText.textContent = "no models configured yet";
    epText.classList.add("muted");
  } else {
    setDot(epDot, "dot-ok");
    epText.textContent = models.join(", ");
    epText.classList.remove("muted");
  }
}

function ensureHostCard(host) {
  let card = hostCards.get(host.id);
  if (card) return card;
  const tmpl = $("host-card-template");
  const node = tmpl.content.firstElementChild.cloneNode(true);
  node.dataset.hostId = host.id;
  const refs = {
    root: node,
    name: node.querySelector(".host-card-name"),
    badge: node.querySelector('[data-role="badge"]'),
    profileDot: node.querySelector('[data-role="profile-dot"]'),
    profileText: node.querySelector('[data-role="profile-text"]'),
    profileDetail: node.querySelector('[data-role="profile-detail"]'),
    runningDot: node.querySelector('[data-role="running-dot"]'),
    runningText: node.querySelector('[data-role="running-text"]'),
    runningDetail: node.querySelector('[data-role="running-detail"]'),
    btnGenerate: node.querySelector('[data-role="generate"]'),
    btnInstall: node.querySelector('[data-role="install"]'),
    btnReverify: node.querySelector('[data-role="reverify"]'),
    prefs: node.querySelector('[data-role="prefs"]'),
    jwtWarn: node.querySelector('[data-role="jwt-warn"]'),
  };
  refs.btnGenerate.addEventListener("click", () =>
    post(`/api/hosts/${encodeURIComponent(host.id)}/profile/generate`, null, append));
  refs.btnReverify.addEventListener("click", () =>
    post(`/api/hosts/${encodeURIComponent(host.id)}/probe`, null, append));
  refs.btnInstall.addEventListener("click", () => {
    const path = refs.btnInstall.dataset.path;
    if (!path) { append(`[${host.id}] No generated profile yet — click Generate first.`); return; }
    post(`/api/hosts/${encodeURIComponent(host.id)}/profile/install`, { path }, append);
  });
  $("hosts-list").append(node);
  hostCards.set(host.id, refs);
  return refs;
}

function renderHostCard(host, snap) {
  const refs = ensureHostCard(host);
  refs.name.textContent = host.display_name;
  if (host.last_generated_profile) {
    refs.btnInstall.disabled = false;
    refs.btnInstall.dataset.path = host.last_generated_profile;
    refs.btnInstall.title = host.last_generated_profile;
  } else {
    refs.btnInstall.disabled = true;
    delete refs.btnInstall.dataset.path;
    refs.btnInstall.title = "Generate first";
  }

  const hs = host.snapshot;
  if (!hs) { setBadge(refs.badge, "probing…", "badge-muted"); return; }

  const profileState = hs.profile_state || { kind: "absent" };
  const missing = profileState.missing_required || [];
  const installed = profileState.kind === "installed";
  const partial = profileState.kind === "partial";
  if (installed) {
    setDot(refs.profileDot, "dot-ok");
    refs.profileText.textContent = "installed";
  } else if (partial) {
    setDot(refs.profileDot, "dot-warn");
    refs.profileText.textContent = `partial (missing: ${missing.join(", ")})`;
  } else {
    setDot(refs.profileDot, "dot-err");
    refs.profileText.textContent = "not installed";
  }
  refs.profileDetail.textContent = hs.profile_source || "—";
  refs.profileDetail.classList.toggle("muted", !hs.profile_source);

  if (hs.host_running) {
    setDot(refs.runningDot, "dot-ok");
    refs.runningText.textContent = "running";
    refs.runningDetail.textContent = (hs.host_processes || []).join(", ") || "process detected";
    refs.runningDetail.classList.remove("muted");
  } else {
    setDot(refs.runningDot, "dot-warn");
    refs.runningText.textContent = "not running";
    refs.runningDetail.textContent = "launch the app to verify routing";
    refs.runningDetail.classList.add("muted");
  }

  const proxyState = (snap.local_proxy?.state || "Unknown").toString();
  let badgeText, badgeCls;
  if (!installed) { badgeText = "profile not installed"; badgeCls = "badge-warn"; }
  else if (partial) { badgeText = "partial"; badgeCls = "badge-warn"; }
  else if (proxyState === "Unconfigured") { badgeText = "awaiting first launch"; badgeCls = "badge-warn"; }
  else if (proxyState === "Listening") { badgeText = "healthy"; badgeCls = "badge-ok"; }
  else { badgeText = "local proxy down"; badgeCls = "badge-err"; }
  setBadge(refs.badge, badgeText, badgeCls);

  const lines = [];
  const keys = hs.profile_keys || {};
  if (Object.keys(keys).length === 0) {
    lines.push("(no keys present)");
  } else {
    for (const [k, v] of Object.entries(keys)) lines.push(`${k} = ${v}`);
  }
  refs.prefs.textContent = lines.join("\n");

  if (snap.cached_token && snap.cached_token.ttl_seconds < 600 && installed) {
    refs.jwtWarn.hidden = false;
    refs.jwtWarn.textContent = `JWT in profile expires in ~${snap.cached_token.ttl_seconds}s — re-generate before it lapses.`;
  } else {
    refs.jwtWarn.hidden = true;
    refs.jwtWarn.textContent = "";
  }
}

function renderHosts(snap) {
  const list = snap.host_apps || [];
  const presentIds = new Set(list.map((h) => h.id));
  for (const [id, refs] of hostCards.entries()) {
    if (!presentIds.has(id)) {
      refs.root.remove();
      hostCards.delete(id);
    }
  }
  if (list.length === 0) {
    const placeholder = $("hosts-list");
    if (placeholder && placeholder.children.length === 0) {
      const empty = document.createElement("div");
      empty.className = "muted host-list-empty";
      empty.textContent = "No host apps registered on this platform.";
      placeholder.replaceChildren(empty);
    }
    return;
  } else {
    const placeholder = $("hosts-list");
    const noHostsMsg = placeholder?.querySelector(":scope > .host-list-empty");
    if (noHostsMsg) noHostsMsg.remove();
  }
  for (const host of list) renderHostCard(host, snap);
}

function renderOverallBadge(snap) {
  const cloudState = (snap.gateway_status?.state || "unknown");
  if (cloudState === "probing" || cloudState === "unknown") {
    setBadge($("overall-badge"), "checking…", "badge-muted"); return;
  }
  if (cloudState === "unreachable") {
    setBadge($("overall-badge"), "cloud unreachable", "badge-err"); return;
  }
  const hosts = snap.host_apps || [];
  if (hosts.length === 0) {
    setBadge($("overall-badge"), "no host apps", "badge-muted"); return;
  }
  const proxyState = (snap.local_proxy?.state || "Unknown").toString();
  const anyAbsent = hosts.some((h) => (h.snapshot?.profile_state?.kind || "absent") === "absent");
  const anyPartial = hosts.some((h) => h.snapshot?.profile_state?.kind === "partial");
  const allInstalled = hosts.every((h) => h.snapshot?.profile_state?.kind === "installed");
  if (anyAbsent)  { setBadge($("overall-badge"), "profile not installed", "badge-warn"); return; }
  if (anyPartial) { setBadge($("overall-badge"), "profile partial", "badge-warn"); return; }
  if (allInstalled && proxyState === "Unconfigured") { setBadge($("overall-badge"), "awaiting first launch", "badge-warn"); return; }
  if (allInstalled && proxyState === "Listening")    { setBadge($("overall-badge"), "healthy", "badge-ok"); return; }
  if (allInstalled) { setBadge($("overall-badge"), "local proxy down", "badge-err"); return; }
  setBadge($("overall-badge"), "checking…", "badge-muted");
}

function renderMarketplaceBadge(snap) {
  const badge = $("marketplace-status");
  badge.classList.remove("badge-muted", "badge-ok", "badge-warn", "badge-err");
  if (!snap.signed_in) { badge.textContent = "sign-in required"; badge.classList.add("badge-warn"); }
  else if (snap.sync_in_flight) { badge.textContent = "syncing"; badge.classList.add("badge-warn"); }
  else if (snap.last_sync_summary) { badge.textContent = "synced"; badge.classList.add("badge-ok"); }
  else { badge.textContent = "never synced"; badge.classList.add("badge-muted"); }
}

function isConfigured(snap) {
  const reachable = snap.gateway_status && snap.gateway_status.state === "reachable";
  const id = snap.verified_identity;
  return !!(reachable && id && id.user_id);
}

function applySetupMode(snap) {
  const setup = !isConfigured(snap);
  document.body.classList.toggle("setup-mode", setup);
  if (!setup) { setSetupError(""); return; }

  const gwInput = $("setup-gateway");
  if (document.activeElement !== gwInput) {
    const next = snap.gateway_url || "";
    if (gwInput.value !== next) {
      gwInput.value = next;
      setLastSavedGateway(next);
    }
    updateSetupPatLink();
  }
  const patInput = $("setup-pat");
  if (snap.pat_present && document.activeElement !== patInput && patInput.dataset.saved !== "1" && patInput.value === "") {
    patInput.value = "•".repeat(24);
    patInput.dataset.saved = "1";
  } else if (!snap.pat_present && patInput.dataset.saved === "1") {
    patInput.value = "";
    delete patInput.dataset.saved;
  }
  const dot = $("setup-gateway-dot");
  const msg = $("setup-gateway-msg");
  dot.classList.remove("dot-unknown", "dot-probing", "dot-ok", "dot-err");
  const status = snap.gateway_status || { state: "unknown" };
  switch (status.state) {
    case "reachable":
      dot.classList.add("dot-ok");
      msg.textContent = `reachable · ${status.latency_ms}ms`;
      msg.classList.remove("muted");
      break;
    case "probing":
      dot.classList.add("dot-probing");
      msg.textContent = "probing…";
      msg.classList.add("muted");
      break;
    case "unreachable":
      dot.classList.add("dot-err");
      msg.textContent = `unreachable · ${status.reason || "unknown error"}`;
      msg.classList.remove("muted");
      break;
    default:
      dot.classList.add("dot-unknown");
      msg.textContent = snap.gateway_url ? "not yet probed" : "enter a URL to probe…";
      msg.classList.add("muted");
  }
  if (status.state === "reachable" && snap.pat_present && !(snap.verified_identity && snap.verified_identity.user_id)) {
    setSetupError("Token rejected by gateway. Issue a fresh PAT and try again.");
  } else if (status.state === "unreachable" && snap.pat_present) {
    setSetupError(`Gateway unreachable: ${status.reason || "unknown error"}`);
  }
}

function fmtCount(n) {
  const v = Number(n) || 0;
  if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(1)}M`;
  if (v >= 1_000) return `${(v / 1_000).toFixed(1)}k`;
  return String(v);
}

let snapshotListeners = [];
export function onSnapshot(fn) { snapshotListeners.push(fn); }

export function applySnapshot(snap) {
  renderCloud(snap);
  renderProxy(snap);
  renderHosts(snap);
  renderOverallBadge(snap);
  renderMarketplaceBadge(snap);
  applySetupMode(snap);

  $("plugins-dir").textContent = snap.plugins_dir || "—";
  $("last-sync").textContent = snap.last_sync_summary || "never synced";
  const mktMeta = $("mkt-meta");
  if (mktMeta) {
    const state = snap.last_sync_summary
      ? (snap.gateway_status && snap.gateway_status.state === "unreachable" ? "err" : "ok")
      : "never";
    mktMeta.dataset.state = state;
    mktMeta.title = snap.last_sync_summary
      ? `Last sync: ${snap.last_sync_summary}\n${snap.plugins_dir || ""}`
      : (snap.plugins_dir || "Run a sync to populate");
  }

  $("config-path").textContent = snap.config_file || "";
  if ($("settings-gateway"))     { $("settings-gateway").textContent     = snap.gateway_url || "—"; $("settings-gateway").classList.toggle("muted", !snap.gateway_url); }
  if ($("settings-plugins-dir")) { $("settings-plugins-dir").textContent = snap.plugins_dir || "—"; $("settings-plugins-dir").classList.toggle("muted", !snap.plugins_dir); }
  if ($("settings-config"))      { $("settings-config").textContent      = snap.config_file || "—"; $("settings-config").classList.toggle("muted", !snap.config_file); }

  const pill = $("sync-pill");
  const pillLabel = pill.querySelector(".sync-pill-label");
  let pillState = "idle";
  let pillText = "needs sign-in";
  if (snap.sync_in_flight) { pillState = "running"; pillText = "syncing"; }
  else if (snap.gateway_status && snap.gateway_status.state === "unreachable") { pillState = "err"; pillText = "offline"; }
  else if (snap.signed_in) { pillState = "ok"; pillText = snap.last_sync_summary ? "synced" : "ready"; }
  pill.dataset.state = pillState;
  pillLabel.textContent = pillText;
  pill.title = snap.last_sync_summary ? `Last sync: ${snap.last_sync_summary}` : "No syncs yet";

  const hostDot = $("footer-host-dot");
  if (hostDot) {
    const cls = pillState === "ok" ? "dot-ok"
              : pillState === "running" ? "dot-probing"
              : pillState === "err" ? "dot-err"
              : "dot-warn";
    setDot(hostDot, cls);
  }

  const id = snap.verified_identity;
  const profileSub = $("rail-profile-sub");
  if (!profileSub.dataset.baseVersion) {
    profileSub.dataset.baseVersion = profileSub.textContent.trim();
  }
  const baseVersion = profileSub.dataset.baseVersion;
  const tenant = id && id.tenant_id;
  profileSub.textContent = tenant ? `${tenant} · ${baseVersion}` : baseVersion;
  $("rail-profile-id").textContent = (id && (id.email || id.user_id)) || "cowork workspace";

  const idSrc = (id && (id.email || id.user_id)) || "";
  const letters = idSrc.replace(/[^a-zA-Z]/g, "").slice(0, 2).toUpperCase();
  $("rail-profile-initials").textContent = letters || "SP";

  $("btn-sync").disabled = !!snap.sync_in_flight || !snap.signed_in;
  $("setup-connect").disabled = !!snap.sync_in_flight;

  const laneStats = snap.proxy_stats || {};
  if ($("lane-msgs")) $("lane-msgs").textContent = fmtCount(laneStats.messages_total);
  if ($("lane-tin"))  $("lane-tin").textContent  = fmtCount(laneStats.tokens_in_total);
  if ($("lane-tout")) $("lane-tout").textContent = fmtCount(laneStats.tokens_out_total);

  for (const fn of snapshotListeners) fn(snap);
  requestAnimationFrame(syncRailIndicator);
}
