import { setDot, setBadge } from "../dom.js?t=__TOKEN__";

export const refsFromNode = (node) => ({
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
});

const applyProfileState = (refs, hs) => {
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
};

const applyRunningState = (refs, hs) => {
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
};

const chooseBadge = (installed, partial, proxyState) => {
  if (!installed) {
    return { text: "profile not installed", cls: "badge-warn" };
  }
  if (partial) {
    return { text: "partial", cls: "badge-warn" };
  }
  if (proxyState === "Unconfigured") {
    return { text: "awaiting first launch", cls: "badge-warn" };
  }
  if (proxyState === "Listening") {
    return { text: "healthy", cls: "badge-ok" };
  }
  return { text: "local proxy down", cls: "badge-err" };
};

const applyPrefs = (refs, hs) => {
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
};

const applyJwtWarn = (refs, snap, installed) => {
  if (snap.cached_token && snap.cached_token.ttl_seconds < 600 && installed) {
    refs.jwtWarn.hidden = false;
    refs.jwtWarn.textContent = `JWT in profile expires in ~${snap.cached_token.ttl_seconds}s — re-generate before it lapses.`;
  } else {
    refs.jwtWarn.hidden = true;
    refs.jwtWarn.textContent = "";
  }
};

const applyInstallButton = (refs, host) => {
  if (host.last_generated_profile) {
    refs.btnInstall.disabled = false;
    refs.btnInstall.dataset.path = host.last_generated_profile;
    refs.btnInstall.title = host.last_generated_profile;
  } else {
    refs.btnInstall.disabled = true;
    delete refs.btnInstall.dataset.path;
    refs.btnInstall.title = "Generate first";
  }
};

export const renderHostCard = (refs, host, snap) => {
  refs.name.textContent = host.display_name;
  applyInstallButton(refs, host);
  const hs = host.snapshot;
  if (!hs) {
    setBadge(refs.badge, "probing…", "badge-muted");
    return;
  }
  const { installed, partial } = applyProfileState(refs, hs);
  applyRunningState(refs, hs);
  const proxyState = ((snap.local_proxy && snap.local_proxy.state) || "Unknown").toString();
  const badge = chooseBadge(installed, partial, proxyState);
  setBadge(refs.badge, badge.text, badge.cls);
  applyPrefs(refs, hs);
  applyJwtWarn(refs, snap, installed);
};
