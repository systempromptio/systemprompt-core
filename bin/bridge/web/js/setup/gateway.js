import { $ } from "../dom.js?t=__TOKEN__";
import { apiPost } from "../api.js?t=__TOKEN__";
import { append, reportError, showToast } from "../drawer.js?t=__TOKEN__";

let gatewayDebounceTimer = null;
let lastSavedGateway = "";

export const getLastSavedGateway = () => lastSavedGateway;

export const setLastSavedGateway = (value) => {
  lastSavedGateway = value;
};

export const setSetupError = (msg) => {
  const el = $("setup-error");
  if (!el) {
    return;
  }
  if (msg) {
    el.textContent = msg;
    el.hidden = false;
  } else {
    el.textContent = "";
    el.hidden = true;
  }
};

const postGateway = async (url) => {
  try {
    await apiPost("/api/gateway", { url });
  } catch (e) {
    reportError(String(e.message || e));
  }
};

export const updateSetupPatLink = () => {
  const link = $("setup-pat-link");
  const gwInput = $("setup-gateway");
  if (!link || !gwInput) {
    return;
  }
  const gw = gwInput.value.trim().replace(/\/+$/, "");
  if (gw) {
    link.href = `${gw}/admin/login`;
    link.removeAttribute("aria-disabled");
    link.classList.remove("is-disabled");
  } else {
    link.href = "#";
    link.setAttribute("aria-disabled", "true");
    link.classList.add("is-disabled");
  }
};

const persistGateway = () => {
  const url = $("setup-gateway").value.trim();
  if (url && url !== lastSavedGateway) {
    lastSavedGateway = url;
    postGateway(url);
  }
};

const scheduleGatewayPersist = () => {
  clearTimeout(gatewayDebounceTimer);
  gatewayDebounceTimer = setTimeout(persistGateway, 600);
};

const flushGatewayPersist = () => {
  clearTimeout(gatewayDebounceTimer);
  persistGateway();
};

export const initGateway = () => {
  const gateway = $("setup-gateway");
  if (gateway) {
    gateway.addEventListener("input", () => {
      updateSetupPatLink();
      scheduleGatewayPersist();
    });
    gateway.addEventListener("blur", flushGatewayPersist);
  }
  const pat = $("setup-pat");
  if (pat) {
    pat.addEventListener("focus", () => {
      if (pat.dataset.saved === "1") {
        pat.value = "";
        delete pat.dataset.saved;
      }
    });
  }
};

let connectPendingSince = 0;

const setConnectPending = (pending) => {
  const btn = document.querySelector('[data-action="setup-connect"], #setup-connect');
  if (!btn) {
    return;
  }
  const label = btn.querySelector(".sp-btn__label") || btn;
  if (pending) {
    if (!btn.dataset.label) {
      btn.dataset.label = label.textContent;
    }
    label.textContent = "Connecting…";
    btn.disabled = true;
    btn.setAttribute("aria-busy", "true");
    connectPendingSince = Date.now();
  } else {
    if (btn.dataset.label) {
      label.textContent = btn.dataset.label;
    }
    btn.disabled = false;
    btn.removeAttribute("aria-busy");
    connectPendingSince = 0;
  }
};

export const isConnectPending = () => connectPendingSince > 0;
export const connectPendingDurationMs = () =>
  connectPendingSince > 0 ? Date.now() - connectPendingSince : 0;
export const clearConnectPending = () => setConnectPending(false);

export const connectFromSetup = () => {
  const input = $("setup-pat");
  const gateway = $("setup-gateway").value.trim();
  if (!gateway) {
    setSetupError("Enter the gateway URL.");
    return;
  }
  if (!/^https?:\/\//i.test(gateway)) {
    setSetupError("Gateway URL must start with http:// or https://");
    return;
  }
  if (input.dataset.saved === "1") {
    setSetupError("");
    lastSavedGateway = gateway;
    setConnectPending(true);
    showToast(`Re-probing ${gateway}…`, "info", 4000);
    append(`Re-probing ${gateway}…`);
    apiPost("/api/probe").catch((e) => {
      const msg = String(e.message || e);
      reportError(msg);
      setSetupError(`Probe failed: ${msg}`);
      setConnectPending(false);
    });
    return;
  }
  const token = input.value.trim();
  if (!token) {
    setSetupError("Paste your personal access token.");
    return;
  }
  setSetupError("");
  lastSavedGateway = gateway;
  setConnectPending(true);
  showToast(`Connecting to ${gateway}…`, "info", 6000);
  append(`Submitting PAT to ${gateway} …`);
  apiPost("/api/login", { token, gateway }).catch((e) => {
    const msg = String(e.message || e);
    reportError(msg);
    setSetupError(`Login failed: ${msg}`);
    setConnectPending(false);
  });
};

export const editSetupPat = () => {
  const input = $("setup-pat");
  if (input && input.dataset.saved === "1") {
    input.value = "";
    delete input.dataset.saved;
    input.focus();
  }
};

export const completeSetupRequest = async () => {
  try {
    await apiPost("/api/setup/complete");
  } catch (e) {
    reportError(String(e.message || e));
  }
};
