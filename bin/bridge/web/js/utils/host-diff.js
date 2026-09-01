// Timestamps advance on every probe whether or not anything changed; comparing
// them would make every tick look like a real update. Mirrors VOLATILE_KEYS on
// the Rust side (src/gui/emit.rs).
const VOLATILE_KEYS = new Set([
  "probed_at_unix",
  "last_probe_at_unix",
  "ttl_seconds",
  "expires_at_unix",
]);

function stableHostJson(value) {
  return JSON.stringify(value, (k, v) => (VOLATILE_KEYS.has(k) ? undefined : v));
}

export function sameHost(a, b) {
  return a != null && b != null && stableHostJson(a) === stableHostJson(b);
}
