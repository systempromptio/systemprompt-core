import { $ } from "./dom.js?t=__TOKEN__";
import { post } from "./api.js?t=__TOKEN__";
import { append } from "./activity.js?t=__TOKEN__";

let gatewayDebounceTimer = null;
let lastSavedGateway = "";

export function setSetupError(msg) {
  const el = $("setup-error");
  if (!el) return;
  if (msg) {
    el.textContent = msg;
    el.hidden = false;
  } else {
    el.textContent = "";
    el.hidden = true;
  }
}

export function getLastSavedGateway() { return lastSavedGateway; }
export function setLastSavedGateway(v) { lastSavedGateway = v; }

export function updateSetupPatLink() {
  const link = $("setup-pat-link");
  if (!link) return;
  const gw = $("setup-gateway").value.trim().replace(/\/+$/, "");
  if (gw) {
    link.href = `${gw}/admin/login`;
    link.removeAttribute("aria-disabled");
    link.classList.remove("disabled");
  } else {
    link.href = "#";
    link.setAttribute("aria-disabled", "true");
    link.classList.add("disabled");
  }
}

function scheduleGatewayPersist() {
  clearTimeout(gatewayDebounceTimer);
  gatewayDebounceTimer = setTimeout(() => {
    const url = $("setup-gateway").value.trim();
    if (!url || url === lastSavedGateway) return;
    lastSavedGateway = url;
    post("/api/gateway", { url }, append);
  }, 600);
}

export function initSetup() {
  $("setup-gateway").addEventListener("input", () => {
    updateSetupPatLink();
    scheduleGatewayPersist();
  });
  $("setup-gateway").addEventListener("blur", () => {
    clearTimeout(gatewayDebounceTimer);
    const url = $("setup-gateway").value.trim();
    if (url && url !== lastSavedGateway) {
      lastSavedGateway = url;
      post("/api/gateway", { url }, append);
    }
  });
  $("setup-pat-link").addEventListener("click", (e) => {
    if ($("setup-pat-link").getAttribute("aria-disabled") === "true") {
      e.preventDefault();
      setSetupError("Enter the gateway URL first.");
    }
  });
  $("setup-pat").addEventListener("focus", () => {
    const input = $("setup-pat");
    if (input.dataset.saved === "1") {
      input.value = "";
      delete input.dataset.saved;
    }
  });
  $("setup-connect").addEventListener("click", () => {
    const input = $("setup-pat");
    const gateway = $("setup-gateway").value.trim();
    if (!gateway) { setSetupError("Enter the gateway URL."); return; }
    if (input.dataset.saved === "1") {
      setSetupError("");
      lastSavedGateway = gateway;
      post("/api/probe", null, append);
      return;
    }
    const token = input.value.trim();
    if (!token) { setSetupError("Paste your personal access token."); return; }
    setSetupError("");
    lastSavedGateway = gateway;
    post("/api/login", { token, gateway }, append);
  });
}
