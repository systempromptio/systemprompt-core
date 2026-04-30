import { $ } from "../dom.js?t=__TOKEN__";
import { renderSetupAgents } from "./agents.js?t=__TOKEN__";
import {
  setSetupError,
  updateSetupPatLink,
  setLastSavedGateway,
} from "./gateway.js?t=__TOKEN__";

export const setSetupStep = (step) => {
  document.body.dataset.setupStep = step;
  const label = $("setup-step-label");
  if (!label) {
    return;
  }
  const map = { connect: "Step 1 of 3", agents: "Step 2 of 3", done: "Step 3 of 3" };
  label.textContent = map[step] || "";
};

export const openSetupMode = () => {
  document.body.classList.add("setup-mode");
};

export const closeSetupMode = () => {
  document.body.classList.remove("setup-mode");
};

const isConfigured = (snap) => {
  const reachable = snap.gateway_status && snap.gateway_status.state === "reachable";
  const id = snap.verified_identity;
  return !!(reachable && id && id.user_id);
};

const syncGatewayInput = (snap) => {
  const gwInput = $("setup-gateway");
  if (!gwInput || document.activeElement === gwInput) {
    return;
  }
  const next = snap.gateway_url || "";
  if (gwInput.value !== next) {
    gwInput.value = next;
    setLastSavedGateway(next);
  }
  updateSetupPatLink();
};

const syncPatInput = (snap) => {
  const patInput = $("setup-pat");
  if (!patInput) {
    return;
  }
  const isFocused = document.activeElement === patInput;
  const isSaved = patInput.dataset.saved === "1";
  if (snap.pat_present && !isFocused && !isSaved && patInput.value === "") {
    patInput.value = "•".repeat(24);
    patInput.dataset.saved = "1";
  } else if (!snap.pat_present && isSaved) {
    patInput.value = "";
    delete patInput.dataset.saved;
  }
};

const setProbeDot = (dot, msg, snap) => {
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
  return status;
};

const setProbeError = (status, snap) => {
  const verified = snap.verified_identity && snap.verified_identity.user_id;
  if (status.state === "reachable" && snap.pat_present && !verified) {
    setSetupError("Token rejected by gateway. Issue a fresh PAT and try again.");
  } else if (status.state === "unreachable" && snap.pat_present) {
    setSetupError(`Gateway unreachable: ${status.reason || "unknown error"}`);
  }
};

const syncGatewayProbe = (snap) => {
  const dot = $("setup-gateway-dot");
  const msg = $("setup-gateway-msg");
  if (!dot || !msg) {
    return;
  }
  const status = setProbeDot(dot, msg, snap);
  setProbeError(status, snap);
};

const renderConnectStep = (snap) => {
  setSetupStep("connect");
  syncGatewayInput(snap);
  syncPatInput(snap);
  syncGatewayProbe(snap);
};

export const applySetupMode = (snap) => {
  const configured = isConfigured(snap);
  const onboarded = snap.agents_onboarded === true;
  const anyInstalled = (snap.host_apps || []).some(
    (h) => h.snapshot?.profile_state?.kind === "installed",
  );
  const setup = !(configured && (onboarded || anyInstalled));
  document.body.classList.toggle("setup-mode", setup);
  if (!setup) {
    setSetupError("");
    return;
  }
  if (configured) {
    setSetupStep("agents");
    renderSetupAgents(snap);
  } else {
    renderConnectStep(snap);
  }
};
