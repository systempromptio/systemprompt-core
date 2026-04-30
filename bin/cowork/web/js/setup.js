import { $ } from "./dom.js?t=__TOKEN__";
import { apiPost } from "./api.js?t=__TOKEN__";
import { append, reportError } from "./drawer.js?t=__TOKEN__";

let gatewayDebounceTimer = null;
let lastSavedGateway = "";

function setSetupError(msg) {
  const el = $("setup-error");
  if (el) {
    if (msg) {
      el.textContent = msg;
      el.hidden = false;
    } else {
      el.textContent = "";
      el.hidden = true;
    }
  }
}

function postGateway(url) {
  apiPost("/api/gateway", { url }).catch((e) => reportError(String(e.message || e)));
}

function updateSetupPatLink() {
  const link = $("setup-pat-link");
  const gwInput = $("setup-gateway");
  if (link && gwInput) {
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
  }
}

function persistGateway() {
  const url = $("setup-gateway").value.trim();
  if (url && url !== lastSavedGateway) {
    lastSavedGateway = url;
    postGateway(url);
  }
}

function scheduleGatewayPersist() {
  clearTimeout(gatewayDebounceTimer);
  gatewayDebounceTimer = setTimeout(persistGateway, 600);
}

export function openSetupMode() {
  document.body.classList.add("setup-mode");
}

export function editSetupPat() {
  const input = $("setup-pat");
  if (input && input.dataset.saved === "1") {
    input.value = "";
    delete input.dataset.saved;
    input.focus();
  }
}

export function connectFromSetup() {
  const input = $("setup-pat");
  const gateway = $("setup-gateway").value.trim();
  if (!gateway) {
    setSetupError("Enter the gateway URL.");
  } else if (input.dataset.saved === "1") {
    setSetupError("");
    lastSavedGateway = gateway;
    apiPost("/api/probe").catch((e) => reportError(String(e.message || e)));
  } else {
    const token = input.value.trim();
    if (!token) {
      setSetupError("Paste your personal access token.");
    } else {
      setSetupError("");
      lastSavedGateway = gateway;
      apiPost("/api/login", { token, gateway }).catch((e) => reportError(String(e.message || e)));
    }
  }
}

export function initSetup() {
  const gateway = $("setup-gateway");
  if (gateway) {
    gateway.addEventListener("input", () => {
      updateSetupPatLink();
      scheduleGatewayPersist();
    });
    gateway.addEventListener("blur", () => {
      clearTimeout(gatewayDebounceTimer);
      persistGateway();
    });
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
}

function isConfigured(snap) {
  const reachable = snap.gateway_status && snap.gateway_status.state === "reachable";
  const id = snap.verified_identity;
  return !!(reachable && id && id.user_id);
}

function syncGatewayInput(snap) {
  const gwInput = $("setup-gateway");
  if (gwInput && document.activeElement !== gwInput) {
    const next = snap.gateway_url || "";
    if (gwInput.value !== next) {
      gwInput.value = next;
      lastSavedGateway = next;
    }
    updateSetupPatLink();
  }
}

function syncPatInput(snap) {
  const patInput = $("setup-pat");
  if (patInput) {
    if (snap.pat_present && document.activeElement !== patInput && patInput.dataset.saved !== "1" && patInput.value === "") {
      patInput.value = "•".repeat(24);
      patInput.dataset.saved = "1";
    } else if (!snap.pat_present && patInput.dataset.saved === "1") {
      patInput.value = "";
      delete patInput.dataset.saved;
    } else {
      void 0;
    }
  }
}

function syncGatewayProbe(snap) {
  const dot = $("setup-gateway-dot");
  const msg = $("setup-gateway-msg");
  if (!dot || !msg) {
    return;
  }
  dot.classList.remove("dot-unknown", "dot-probing", "dot-ok", "dot-err");
  const status = snap.gateway_status || { state: "unknown" };
  if (status.state === "reachable") {
    dot.classList.add("dot-ok");
    msg.textContent = `reachable · ${status.latency_ms}ms`;
    msg.classList.remove("muted");
  } else if (status.state === "probing") {
    dot.classList.add("dot-probing");
    msg.textContent = "probing…";
    msg.classList.add("muted");
  } else if (status.state === "unreachable") {
    dot.classList.add("dot-err");
    msg.textContent = `unreachable · ${status.reason || "unknown error"}`;
    msg.classList.remove("muted");
  } else {
    dot.classList.add("dot-unknown");
    msg.textContent = snap.gateway_url ? "not yet probed" : "enter a URL to probe…";
    msg.classList.add("muted");
  }
  if (status.state === "reachable" && snap.pat_present && !(snap.verified_identity && snap.verified_identity.user_id)) {
    setSetupError("Token rejected by gateway. Issue a fresh PAT and try again.");
  } else if (status.state === "unreachable" && snap.pat_present) {
    setSetupError(`Gateway unreachable: ${status.reason || "unknown error"}`);
  } else {
    void 0;
  }
}

export function applySetupMode(snap) {
  const setup = !isConfigured(snap);
  document.body.classList.toggle("setup-mode", setup);
  if (setup) {
    syncGatewayInput(snap);
    syncPatInput(snap);
    syncGatewayProbe(snap);
  } else {
    setSetupError("");
  }
}
