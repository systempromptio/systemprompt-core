import { $, setDot, setBadge } from "./dom.js?t=__TOKEN__";

const hostCards = new Map();

function refsFromNode(node) {
  return {
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
}

export function getOrCreateHostCard(id) {
  let refs = hostCards.get(id);
  if (refs) {
    return refs;
  }
  const tmpl = $("host-card-template");
  if (!tmpl) {
    return null;
  }
  const node = tmpl.content.firstElementChild.cloneNode(true);
  node.dataset.hostId = id;
  refs = refsFromNode(node);
  if (refs.btnGenerate) {
    refs.btnGenerate.dataset.action = "host-generate";
    refs.btnGenerate.dataset.host = id;
  }
  if (refs.btnReverify) {
    refs.btnReverify.dataset.action = "host-reverify";
    refs.btnReverify.dataset.host = id;
  }
  if (refs.btnInstall) {
    refs.btnInstall.dataset.action = "host-install";
    refs.btnInstall.dataset.host = id;
  }
  $("hosts-list").append(node);
  hostCards.set(id, refs);
  return refs;
}

function applyProfileState(refs, hs) {
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
  return { installed, partial };
}

function applyRunningState(refs, hs) {
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
}

function chooseBadge(installed, partial, proxyState) {
  if (!installed) {
    return { text: "profile not installed", cls: "badge-warn" };
  } else if (partial) {
    return { text: "partial", cls: "badge-warn" };
  } else if (proxyState === "Unconfigured") {
    return { text: "awaiting first launch", cls: "badge-warn" };
  } else if (proxyState === "Listening") {
    return { text: "healthy", cls: "badge-ok" };
  } else {
    return { text: "local proxy down", cls: "badge-err" };
  }
}

function applyPrefs(refs, hs) {
  const lines = [];
  const keys = hs.profile_keys || {};
  if (Object.keys(keys).length === 0) {
    lines.push("(no keys present)");
  } else {
    for (const [k, v] of Object.entries(keys)) {
      lines.push(`${k} = ${v}`);
    }
  }
  refs.prefs.textContent = lines.join("\n");
}

function applyJwtWarn(refs, snap, installed) {
  if (snap.cached_token && snap.cached_token.ttl_seconds < 600 && installed) {
    refs.jwtWarn.hidden = false;
    refs.jwtWarn.textContent = `JWT in profile expires in ~${snap.cached_token.ttl_seconds}s — re-generate before it lapses.`;
  } else {
    refs.jwtWarn.hidden = true;
    refs.jwtWarn.textContent = "";
  }
}

export function renderHostCard(refs, host, snap) {
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
  if (!hs) {
    setBadge(refs.badge, "probing…", "badge-muted");
  } else {
    const { installed, partial } = applyProfileState(refs, hs);
    applyRunningState(refs, hs);
    const proxyState = (snap.local_proxy && snap.local_proxy.state || "Unknown").toString();
    const badge = chooseBadge(installed, partial, proxyState);
    setBadge(refs.badge, badge.text, badge.cls);
    applyPrefs(refs, hs);
    applyJwtWarn(refs, snap, installed);
  }
}

function renderEmptyHosts(list) {
  if (list && list.children.length === 0) {
    const empty = document.createElement("div");
    empty.className = "muted host-list-empty";
    empty.textContent = "No host apps registered on this platform.";
    list.replaceChildren(empty);
  }
}

export function renderHosts(snap) {
  const list = snap.host_apps || [];
  const presentIds = new Set(list.map((h) => h.id));
  for (const [id, refs] of hostCards.entries()) {
    if (!presentIds.has(id)) {
      refs.root.remove();
      hostCards.delete(id);
    }
  }
  const placeholder = $("hosts-list");
  if (list.length === 0) {
    renderEmptyHosts(placeholder);
  } else {
    const noHostsMsg = placeholder && placeholder.querySelector(":scope > .host-list-empty");
    if (noHostsMsg) {
      noHostsMsg.remove();
    }
    for (const host of list) {
      const refs = getOrCreateHostCard(host.id);
      if (refs) {
        renderHostCard(refs, host, snap);
      }
    }
  }
}
