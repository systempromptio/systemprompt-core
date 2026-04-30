# systemprompt-bridge en-US message catalog.
# Add new locale files at web/i18n/<locale>/bridge.ftl. Fall back to en-US.

ready = Ready.

# Setup wizard ----------------------------------------------------------------
setup-heading = Welcome to systemprompt bridge
setup-lede = systemprompt bridge routes one or more coding agents through your enterprise gateway. Connect this device, pick the agents you want governed, and you're done.
setup-eyebrow-prefix = DEMO BUILD
setup-gateway-label = Gateway URL
setup-gateway-empty = enter a URL to probe…
setup-pat-label = Personal access token
setup-pat-hint = Don't have one yet?
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
sync-in-flight = syncing
sync-cancel = Cancel

# Gateway ---------------------------------------------------------------------
gateway-unreachable = offline
gateway-not-signed-in = needs sign-in

# Marketplace -----------------------------------------------------------------
marketplace-heading = Marketplace
marketplace-categories = Categories
marketplace-search-placeholder = Search…
marketplace-empty-title = Select an item
marketplace-action-validate = Validate
marketplace-action-open-folder = Open folder
last-sync-never = never synced
last-sync = Last sync: { $summary }

# Agents tab ------------------------------------------------------------------
agents-heading = Agents
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
brand-workspace-pill = bridge workspace

# Marketplace badges / detail / empty -----------------------------------------
marketplace-badge-signin = sign-in required
marketplace-badge-syncing = syncing
marketplace-badge-synced = synced
marketplace-badge-never = never synced
marketplace-detail-readme = README
marketplace-detail-path = Path
marketplace-detail-copy = Copy
marketplace-detail-copied = Copied ✓

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
setup-agents-empty = No agents available on this platform.

# Setup gateway probe ---------------------------------------------------------
setup-gateway-reachable = reachable · { $latency }ms
setup-gateway-probing = probing…
setup-gateway-unreachable = unreachable · { $reason }
setup-connecting = Connecting…
setup-step-label-connect = Step 1 of 3
setup-step-label-agents = Step 2 of 3
setup-step-label-done = Step 3 of 3
setup-gateway-not-probed = not yet probed

# Native menu bar ------------------------------------------------------------
menu-edit = Edit
menu-view = View
menu-help = Help
menu-show-settings = Show settings window
menu-open-log-folder = Open log folder
menu-export-bundle = Export diagnostic bundle
menu-open-config = Open config folder
