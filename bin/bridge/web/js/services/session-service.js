import { bridge } from "/assets/js/bridge.js";

/**
 * Sign out of systemprompt cloud.
 *
 * Clears the stored PAT and cached JWT via the bridge, after which the next
 * state.changed carries no verified identity and `sp-setup` returns the app to
 * the connect splash.
 *
 * Shared so every entry point (status card, rail profile menu) behaves
 * identically. Resolves to an error message, or "" on success — callers render
 * it inline rather than throwing.
 */
export async function logout() {
  try {
    await bridge.logout();
    return "";
  } catch (e) {
    return (e && e.message) || "logout failed";
  }
}
