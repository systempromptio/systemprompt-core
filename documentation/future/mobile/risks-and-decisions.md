# Risks and decisions

## 1. What was verified, and what was not

Nothing in this set has been built or run.

**Read in core** (`future` at `2b7e8dd3d`): everything in `architecture/01-current-state.md`.

**Read on the vendors' own pages, 2026-11-26:**

| Fact | Status |
|---|---|
| Custom apps are offered to organisations the publisher names, through Apple Business Manager or Apple School Manager; they pass App Review; the organisation distributes by device management or redemption codes | Confirmed |
| TestFlight: up to 10,000 external testers; up to 100 internal; external builds are reviewed | Confirmed |
| TestFlight build lifetime (commonly cited as 90 days) | **Not confirmed** on the page read |
| Enterprise program: employees only; 100 or more employees; a legal entity; a verification interview; 299 USD a year | Confirmed |
| Ad hoc device cap per year | **Not confirmed**; the page did not load its text |
| Web distribution in the EU: EU users only; authorisation with financial or scale criteria; notarisation; a 5% commission | Confirmed |
| Android developer verification: in force from 30 September 2026 in four countries for participating stores; direct sideloading not yet covered; worldwide for all apps on certified devices in 2027 | Confirmed |
| A verification path for developers outside the public store, with registration of package names and signing keys; an advanced flow and the debug bridge remain for unverified installs; managed-store enterprise apps exempt | Confirmed |
| Managed private apps: published to an organisation's own store, account created for the organisation at no fee | Confirmed (support page) |

**From search summaries only, not read at source:** the desktop window toolkit's phone status (requests closed as not planned; a third-party port in early development); the binding generator's current version and feature set.

**Assumed:** that the pinned binding generator supports every listed target and async listener callbacks as specified; that each host adapter with a permission hook can pause and resume a run; that hosted macOS runners can run the simulator UI tests and hosted Linux runners the emulator UI tests within reasonable time.

M0-A1 closes the unconfirmed and assumed items.

## 2. Risks

| # | Risk | Why it matters | Mitigation |
|---|---|---|---|
| 1 | iOS cannot be self-hosted | The stated preference was to avoid the store | The chosen route is private and never listed; the limits are stated to installs up front; an enterprise membership remains possible for a customer that has one |
| 2 | A customer without Apple Business Manager | The chosen iOS route does not reach them | Beta channel for a bounded group; otherwise that customer's own enterprise membership |
| 3 | App Review refuses or delays | Custom apps and external beta builds are reviewed; a client for a private server can be hard to demonstrate | A review account and sample data on the reference instance; review notes kept with the packaging |
| 4 | Android verification widens | Self-hosted installs would be blocked for an unregistered publisher | Register the publisher, package names and signing keys before the worldwide date |
| 5 | Push credentials per instance | An instance whose app is published by someone else has no credentials and gets no push | Documented; the app works without push; a relay is the alternative that was declined |
| 6 | Push payload leakage | Notification content would pass through third parties | The payload carries an id and a category only; reviewed; asserted in scenario 17 |
| 7 | The approval step is new authorization surface | A flaw lets the wrong person, or no one, allow a tool | Owner only; timeout denies; first decision wins; policy before people; independent review; property tests |
| 8 | Hosts that cannot pause | A run could be executed permissively without anyone deciding | Refused at queue time; never downgraded silently |
| 9 | Two apps drift apart | Each screen is written twice | One brief per screen group; a shared screen-and-state list; scenarios must pass on both; the tour side by side |
| 10 | Behaviour leaks into the apps | Fixes would need making three times | Screen models in Rust hold all state and stream handling; a lint rule against networking, token handling and cursor logic in app code |
| 11 | Binding generator limits | An unsupported construct forces a redesign of the surface | M0 confirms the pinned version; the surface is small; M2 wave A proves async and listeners before the surface is filled in |
| 12 | Secrets across the boundary | A token exposed to the native side can be logged or leaked | No secret type is exported; reviewed; a gate on the surface |
| 13 | Runner time and reliability | Simulator and emulator tests are slow and can be unstable | UI tests only at stage gates; scenarios through the core catch most defects first; a repeating failure gets its own lane |
| 14 | Dependence on harness stages | M2 needs the device crate in core; M3's assets need the brand and theme data | M1 needs nothing and starts first; M3-C7 can begin from the schema before harness S3 |
| 15 | Three languages to maintain | More skills and tooling in the template and every install | Core stays Rust; the apps hold presentation only; structure and lint rules are the same shape on both |
| 16 | Phones cannot take contributed screens | The desktop's mount-point model does not carry over | Stated plainly; forks edit their native code; three generic server-driven layouts are proposed (open decision O1) |
| 17 | Signing secrets in release workflows | A leaked key lets someone ship as the publisher | Secrets only in release environments; push runs unsigned; each publisher holds its own |

## 3. Decisions taken

| # | Decision | Alternative declined |
|---|---|---|
| M1 | The phone is a client of server-side runs only | Attaching to a desktop session; an on-device agent loop |
| M2 | A shared Rust core with SwiftUI and Jetpack Compose | One cross-platform UI codebase; a Rust-only UI on an early phone port of the desktop toolkit |
| M3 | iOS through custom apps and the beta channel; Android as a self-hosted APK | Enterprise in-house per install; a web app on iOS; supporting every route |
| M4 | Each instance sends its own push | A relay operated centrally; no push |
| M5 | One new core crate, `systemprompt-mobile`; everything else extends existing crates | A feature of an existing crate (not possible: the binding layer needs its own library build) |
| M6 | No new host; the existing iOS and Android client ids identify the apps | A host variant per platform |
| M7 | Register as a verified Android developer now | Rely on sideloading staying uncovered |
| M8 | Pairing reuses the existing device-link exchange, shown as a QR code | A new pairing protocol |
| M9 | Server-sent events only | Adding a WebSocket transport |
| M10 | Push payload carries no content | Rich notifications with server-supplied text |
| M11 | A timed-out approval is denied | Allow on timeout; wait forever |
| M12 | Evidence on a real Mac by an attached agent session; hosted runs store nothing | Stored artefacts from hosted runs |
| M13 | Generated Swift and Kotlin are build outputs, not committed | Committing bindings in core or the template |
| M14 | Screen models live in Rust, one per feature, shared by the desktop harness and both phone apps; only drawing is per platform | The core handing raw facts to each app, with screen logic written three times |
| M15 | Every feature (chat, runs, approvals, inbox, documents, preferences) is an opt-in slice: chosen at build, switchable by the instance, with navigation built from what is admitted | A fixed feature set |
| M16 | No administration on the phone; settings are the user's own preferences only | Admin and governance screens on the phone |
| M17 | The full feature set is the scope; there is no reduced first cut | Shipping a subset first |

## 4. Decisions still open

| # | Decision | Recommendation | Needed by |
|---|---|---|---|
| O1 | Whether to include three generic server-driven layouts (list, detail, form) so a new instance capability can appear on phones without an app release | Yes, but after M5: it is new scope | After M5 |
| O2 | The default approval timeout | Fifteen minutes, configurable per instance | M1 |
| O3 | The reference app's name, icon, bundle identifier and package name | From the owner | M0 |
| O4 | Who holds the developer accounts and signing secrets for the reference app | The maintainers; each install its own | M0 |
| O5 | Whether tablets get an adapted layout | Not in these stages; the phone layout runs on them | After M5 |
