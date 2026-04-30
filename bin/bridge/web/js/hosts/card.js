import { setDot, setBadge } from "../dom.js?t=__TOKEN__";
import { t } from "../i18n.js?t=__TOKEN__";

export const refsFromNode = (node) => ({
  root: node,
  name: node.querySelector(".sp-host-card__name"),
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
    setDot(refs.profileDot, "sp-dot--ok");
    refs.profileText.textContent = t("host-profile-installed");
  } else if (partial) {
    setDot(refs.profileDot, "sp-dot--warn");
    refs.profileText.textContent = t("host-profile-partial", { missing: missing.join(", ") });
  } else {
    setDot(refs.profileDot, "sp-dot--err");
    refs.profileText.textContent = t("host-profile-not-installed");
  }
  refs.profileDetail.textContent = hs.profile_source || "—";
  refs.profileDetail.classList.toggle("sp-u-muted", !hs.profile_source);
  return { installed, partial };
};

const applyRunningState = (refs, hs) => {
  if (hs.host_running) {
    setDot(refs.runningDot, "sp-dot--ok");
    refs.runningText.textContent = t("host-process-running");
    refs.runningDetail.textContent = (hs.host_processes || []).join(", ") || t("host-process-detected");
    refs.runningDetail.classList.remove("sp-u-muted");
  } else {
    setDot(refs.runningDot, "sp-dot--warn");
    refs.runningText.textContent = t("host-process-not-running");
    refs.runningDetail.textContent = t("host-process-detail");
    refs.runningDetail.classList.add("sp-u-muted");
  }
};

const chooseBadge = (installed, partial, proxyState) => {
  if (!installed) {
    return { text: t("host-badge-not-installed"), cls: "sp-badge--warn" };
  }
  if (partial) {
    return { text: t("host-badge-partial"), cls: "sp-badge--warn" };
  }
  if (proxyState === "Unconfigured") {
    return { text: t("host-badge-awaiting"), cls: "sp-badge--warn" };
  }
  if (proxyState === "Listening") {
    return { text: t("host-badge-healthy"), cls: "sp-badge--ok" };
  }
  return { text: t("host-badge-proxy-down"), cls: "sp-badge--err" };
};

const applyPrefs = (refs, hs) => {
  const lines = [];
  const keys = hs.profile_keys || {};
  if (Object.keys(keys).length === 0) {
    lines.push(t("host-prefs-empty"));
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
    refs.jwtWarn.textContent = t("host-jwt-warn", { ttl: snap.cached_token.ttl_seconds });
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
    setBadge(refs.badge, "probing…", "sp-badge--muted");
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
