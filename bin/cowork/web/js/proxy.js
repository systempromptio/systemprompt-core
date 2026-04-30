import { $, setDot } from "./dom.js?t=__TOKEN__";

function collectInferenceModels(snap) {
  const seen = new Set();
  const out = [];
  for (const host of (snap.host_apps || [])) {
    const raw = host.snapshot && host.snapshot.profile_keys && host.snapshot.profile_keys.inferenceModels;
    if (raw) {
      for (const m of raw.split(",")) {
        const t = m.trim();
        if (t && !seen.has(t)) {
          seen.add(t);
          out.push(t);
        }
      }
    }
  }
  return out;
}

function applyProxyStatus(proxy) {
  const state = (proxy.state || "Unknown").toString();
  const dot = $("proxy-dot");
  const text = $("proxy-text");
  let cls = "dot-unknown";
  let label = state;
  if (state === "Listening") {
    cls = "dot-ok";
    label = `listening · ${proxy.latency_ms ?? "?"}ms`;
  } else if (state === "Refused") {
    cls = "dot-err";
    label = "connection refused";
  } else if (state === "Timeout") {
    cls = "dot-err";
    label = "timed out";
  } else if (state === "HttpError") {
    cls = "dot-err";
    label = `error: ${proxy.error || "unknown"}`;
  } else if (state === "Unconfigured") {
    cls = "dot-warn";
    label = "awaiting first host-app probe";
  } else {
    cls = "dot-unknown";
    label = "checking…";
  }
  setDot(dot, cls);
  if (text) {
    text.textContent = label;
  }
  const detail = $("proxy-detail");
  if (detail) {
    detail.textContent = proxy.url || "(no proxy URL configured yet)";
    detail.classList.toggle("muted", !proxy.url);
  }
}

export function renderProxy(snap) {
  applyProxyStatus(snap.local_proxy || { state: "Unknown" });
  const models = collectInferenceModels(snap);
  const epDot = $("endpoints-dot");
  const epText = $("endpoints-text");
  if (models.length === 0) {
    setDot(epDot, "dot-unknown");
    if (epText) {
      epText.textContent = "no models configured yet";
      epText.classList.add("muted");
    }
  } else {
    setDot(epDot, "dot-ok");
    if (epText) {
      epText.textContent = models.join(", ");
      epText.classList.remove("muted");
    }
  }
}
