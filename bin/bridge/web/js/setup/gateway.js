import { $ } from "../dom.js?t=__TOKEN__";
import { apiPost } from "../api.js?t=__TOKEN__";
import { reportError } from "../drawer.js?t=__TOKEN__";

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
    link.classList.remove("disabled");
  } else {
    link.href = "#";
    link.setAttribute("aria-disabled", "true");
    link.classList.add("disabled");
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

export const connectFromSetup = () => {
  const input = $("setup-pat");
  const gateway = $("setup-gateway").value.trim();
  if (!gateway) {
    setSetupError("Enter the gateway URL.");
    return;
  }
  if (input.dataset.saved === "1") {
    setSetupError("");
    lastSavedGateway = gateway;
    apiPost("/api/probe").catch((e) => reportError(String(e.message || e)));
    return;
  }
  const token = input.value.trim();
  if (!token) {
    setSetupError("Paste your personal access token.");
    return;
  }
  setSetupError("");
  lastSavedGateway = gateway;
  apiPost("/api/login", { token, gateway }).catch((e) => reportError(String(e.message || e)));
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
