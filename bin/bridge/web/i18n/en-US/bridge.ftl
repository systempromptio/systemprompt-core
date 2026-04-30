# systemprompt-bridge en-US message catalog.
# Add new locale files at web/i18n/<locale>/bridge.ftl. Fall back to en-US.

ready = Ready.

# Setup wizard ----------------------------------------------------------------
setup-heading = Welcome to systemprompt bridge
setup-lede = systemprompt bridge routes one or more coding agents through your enterprise gateway. Connect this device, pick the agents you want governed, and you're done.
setup-step-label = Step { $current } of { $total }
setup-eyebrow-prefix = DEMO BUILD
setup-gateway-label = Gateway URL
setup-gateway-placeholder = http://127.0.0.1:8080
setup-gateway-empty = enter a URL to probe…
setup-pat-label = Personal access token
setup-pat-placeholder = sp-live-…
setup-pat-hint = Don't have one yet?
setup-pat-link = Open the gateway admin login →
setup-pat-detail = Sign in there, issue a device PAT, then paste it above.
setup-agents-lede = Pick the coding agents you want systemprompt bridge to govern. You can install more later from the Agents tab.
setup-skip-agents = Skip — set up later
setup-finish = Finish
setup-done-lede = systemprompt bridge is ready. Open the workspace to manage agents, sync the marketplace, and watch the proxy.
setup-open = Open systemprompt bridge
setup-warning-strong = Demo software.
setup-warning-body = This build of systemprompt bridge is provided for demonstration purposes only and is not licensed for production use.

# Sync / actions --------------------------------------------------------------
sync-button = Sync now
sync-success = synced
sync-failure = Sync failed: { $error }
sync-in-flight = syncing
sync-cancel = Cancel
sync-cancelled = Sync cancelled

# Auth / login ----------------------------------------------------------------
login-button = Connect
login-success = Connected as { $email }
login-failure = login failed: { $error }
login-cancelled = Login cancelled
login-pat-empty = PAT is empty
login-saving = Saving PAT…
login-stored = PAT stored.
login-pull-manifest = PAT stored. Pulling manifest…

logout-button = Sign out
logout-running = Logging out…
logout-success = Logged out.
logout-failure = logout failed: { $error }

# Gateway ---------------------------------------------------------------------
gateway-saving = Saving gateway URL { $url }…
gateway-saved = Gateway URL saved.
gateway-set-empty = Set gateway: URL is empty
gateway-set-failure = set gateway failed: { $error }
gateway-checking = Checking gateway…
gateway-unreachable = offline
gateway-signed-in = Signed in as { $label }
gateway-not-signed-in = needs sign-in
gateway-pat-stored = PAT stored — verifying…

# Validate --------------------------------------------------------------------
validate-button = Validate
validate-running = Running validation…
validate-failure = Validation failed: { $error }

# Marketplace -----------------------------------------------------------------
marketplace-heading = Marketplace
marketplace-categories = Categories
marketplace-cat-plugins = Plugins
marketplace-cat-skills = Skills
marketplace-cat-hooks = Hooks
marketplace-cat-mcp = MCP servers
marketplace-cat-agents = Agents
marketplace-search-placeholder = Search…
marketplace-empty-title = Select an item
marketplace-action-validate = Validate
marketplace-action-open-folder = Open folder
last-sync-never = never synced
last-sync = Last sync: { $summary }
sync-meta-no-syncs = No syncs yet

# Agents tab ------------------------------------------------------------------
agents-heading = Agents
agents-checking = checking…
agents-lede = systemprompt bridge routes any number of coding agents through a single local proxy. Install the configuration profile for each agent you want governed; they all run simultaneously.

# Status tab ------------------------------------------------------------------
status-heading = Status
status-cloud-heading = systemprompt cloud
status-cloud-caption = The hosted control plane systemprompt bridge talks to. Identity and PAT live here.
status-proxy-heading = Local proxy
status-proxy-caption = The 127.0.0.1 endpoint host apps call instead of the Anthropic API.
status-proxy-health = Health
status-proxy-endpoints = Inference endpoints
status-proxy-endpoints-detail = Models the proxy advertises to host apps.
status-agents-heading = Agents
status-agents-caption-prefix = Coding agents routed through systemprompt bridge. Manage them in the
status-agents-tab-link = Agents tab
status-agents-connected = Connected
status-open-agents = Open agents
status-checking = checking…
status-host-profile-key = Configuration profile
status-host-process = Process
status-host-generate = Generate
status-host-install = Install
status-host-reverify = Re-verify
status-host-prefs-summary = Resolved profile keys

# Settings tab ----------------------------------------------------------------
settings-heading = Settings
settings-gateway-label = Gateway URL
settings-plugins-label = Plugins directory
settings-config-label = Config file
settings-schedule-label = Sync schedule
settings-schedule-value = manual (trigger from Marketplace)
settings-action-open-folder = Open config folder
settings-action-validate = Run validate
settings-action-change-gateway = Change gateway
settings-licensing-note-prefix = Demo build — for production licensing contact

# Activity drawer / footer ----------------------------------------------------
activity-title = Activity
activity-msgs = msgs
activity-tin = in
activity-tout = out
activity-help-title = Help & Support
activity-open-log-folder = Open log folder
activity-export-bundle = Export diagnostic bundle
footer-docs = docs
footer-licensing = licensing
footer-tabs-hint = tabs

# Topbar / navigation ---------------------------------------------------------
nav-marketplace = Marketplace
nav-agents = Agents
nav-status = Status
nav-settings = Settings
nav-section-navigate = Navigate
sync-pill-label = Sync status
agent-presence-label = Connected agents
brand-workspace-pill = bridge workspace

# Toasts / errors -------------------------------------------------------------
toast-dismiss = Dismiss

# Marketplace badges / detail / empty -----------------------------------------
marketplace-badge-signin = sign-in required
marketplace-badge-syncing = syncing
marketplace-badge-synced = synced
marketplace-badge-never = never synced
marketplace-detail-readme = README
marketplace-detail-path = Path
marketplace-detail-copy = Copy
marketplace-detail-copied = Copied ✓
marketplace-empty-search-sub = Try a different term, or clear the search.
marketplace-empty-presync-sub = Run a sync to populate the marketplace.
marketplace-empty-sync-button = Sync now

# Hosts (platform host apps) --------------------------------------------------
hosts-empty = No host apps registered on this platform.
host-profile-installed = installed
host-profile-partial = partial (missing: { $missing })
host-profile-not-installed = not installed
host-process-running = running
host-process-not-running = not running
host-process-detail = launch the app to verify routing
host-jwt-warn = JWT in profile expires in ~{ $ttl }s — re-generate before it lapses.
host-process-detected = process detected
host-prefs-empty = (no keys present)
host-badge-not-installed = profile not installed
host-badge-partial = partial
host-badge-awaiting = awaiting first launch
host-badge-healthy = healthy
host-badge-proxy-down = local proxy down

# Agents (tab summary + setup) ------------------------------------------------
agents-summary-none = no agents registered
agents-summary-count = { $installed } of { $total } agents configured · { $running } running
setup-agents-empty = No agents available on this platform.
agent-presence-running = running
agent-presence-needs-attention = needs attention
agent-presence-not-installed = not installed
agent-presence-unknown = unknown

# Proxy -----------------------------------------------------------------------
proxy-no-models = no models configured yet

# Setup gateway probe ---------------------------------------------------------
setup-gateway-reachable = reachable · { $latency }ms
setup-gateway-probing = probing…
setup-gateway-unreachable = unreachable · { $reason }
setup-gateway-unknown-error = unknown error
setup-connecting = Connecting…
setup-step-label-connect = Step 1 of 3
setup-step-label-agents = Step 2 of 3
setup-step-label-done = Step 3 of 3
setup-gateway-not-probed = not yet probed

# Menu bar (native) -----------------------------------------------------------
menu-edit = Edit
menu-view = View
menu-help = Help
menu-show-settings = Show settings
menu-open-log-folder = Open log folder
menu-export-bundle = Export diagnostic bundle…
menu-open-config = Open config folder

# Quit ------------------------------------------------------------------------
quit = Quit
open-settings = Open settings…
open-config-folder = Open config folder
open-log-folder = Open log folder
export-bundle = Export diagnostic bundle…
