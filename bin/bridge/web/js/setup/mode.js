import { $ } from "../dom.js?t=__TOKEN__";
import { t } from "../i18n.js?t=__TOKEN__";
import { renderSetupAgents } from "./agents.js?t=__TOKEN__";
import {
  setSetupError,
  updateSetupPatLink,
  setLastSavedGateway,
  isConnectPending,
  connectPendingDurationMs,
  clearConnectPending,
} from "./gateway.js?t=__TOKEN__";

export const setSetupStep = (step) => {
  document.body.dataset.setupStep = step;
  const label = $("setup-step-label");
  if (!label) {
    return;
  }
  const map = {
    connect: t("setup-step-label-connect"),
    agents: t("setup-step-label-agents"),
    done: t("setup-step-label-done"),
  };
  label.textContent = map[step] || "";
};

export const openSetupMode = () => {
  document.body.classList.add("is-setup-mode");
};

export const closeSetupMode = () => {
  document.body.classList.remove("is-setup-mode");
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
  dot.classList.remove("sp-dot--unknown", "sp-dot--probing", "sp-dot--ok", "sp-dot--err");
  const status = snap.gateway_status || { state: "unknown" };
  if (status.state === "reachable") {
    dot.classList.add("sp-dot--ok");
    msg.textContent = t("setup-gateway-reachable", { latency: status.latency_ms });
    msg.classList.remove("sp-u-muted");
  } else if (status.state === "probing") {
    dot.classList.add("sp-dot--probing");
    msg.textContent = t("setup-gateway-probing");
    msg.classList.add("sp-u-muted");
  } else if (status.state === "unreachable") {
    dot.classList.add("sp-dot--err");
    msg.textContent = t("setup-gateway-unreachable", { reason: status.reason || t("setup-gateway-unknown-error") });
    msg.classList.remove("sp-u-muted");
  } else {
    dot.classList.add("sp-dot--unknown");
    msg.textContent = snap.gateway_url ? t("setup-gateway-not-probed") : t("setup-gateway-empty");
    msg.classList.add("sp-u-muted");
  }
  return status;
};

const setProbeError = (status, snap) => {
  const verified = snap.verified_identity && snap.verified_identity.user_id;
  const msg = snap.last_action_message || "";
  const looksLikeFailure = /^(login|set gateway|pat|gateway PAT|sync|probe).*(fail|error|refused|invalid)/i.test(
    msg,
  );
  if (looksLikeFailure) {
    setSetupError(msg);
    return;
  }
  if (status.state === "reachable" && snap.pat_present && !verified) {
    setSetupError("Token rejected by gateway. Issue a fresh PAT and try again.");
  } else if (status.state === "unreachable" && snap.pat_present) {
    setSetupError(`Gateway unreachable: ${status.reason || "unknown error"}`);
  } else {
    setSetupError("");
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
  document.body.classList.toggle("is-setup-mode", setup);

  if (isConnectPending()) {
    const probeState = (snap.gateway_status && snap.gateway_status.state) || "unknown";
    const isResolved =
      configured ||
      probeState === "reachable" ||
      probeState === "unreachable" ||
      (snap.last_action_message && /(fail|error|reject|invalid)/i.test(snap.last_action_message));
    if (isResolved || connectPendingDurationMs() > 15000) {
      clearConnectPending();
    }
  }

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
