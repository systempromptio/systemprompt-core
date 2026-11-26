# 04. The shared core and its bindings

Status: specification draft. The Rust below is shape, not final signatures.

## 1. How Swift and Kotlin use Rust

They do not import Rust source. Rust is compiled into a native library for each phone, and a generator writes the Swift and Kotlin code that calls it.

1. The types and functions to expose are annotated in `systemprompt-mobile`.
2. The crate is compiled for each phone architecture: a static library for iOS, shared libraries for Android.
3. UniFFI reads the annotations and generates a Swift module and a Kotlin package.
4. The iOS libraries are wrapped as one XCFramework; the Android libraries as one AAR.
5. The apps depend on those like any other package and call ordinary methods.

Generated Swift and Kotlin are build outputs. They are not committed in core or in the template.

## 2. Targets

| Platform | Rust targets |
|---|---|
| iOS device | `aarch64-apple-ios` |
| iOS simulator | `aarch64-apple-ios-sim`, `x86_64-apple-ios` |
| Android | `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android` |

## 3. The exposed surface

Small on purpose. Everything else in core stays behind it.

| Object | Offers |
|---|---|
| `Instances` | Discover an instance from a URL; list known instances; the active one |
| `SignIn` | Begin and complete authorization code with PKCE; begin and complete a passkey ceremony; redeem a pairing code |
| `Session` | The signed-in user; sign out; the sync state; the applied policy |
| `Agents` | List agents; open or continue a chat; send a message; watch a chat |
| `Runs` | Queue; list; get; cancel; watch events with resume; outputs |
| `Approvals` | Pending requests; decide |
| `Notifications` | Feed; unread count; mark read; register and revoke the push token; resolve a notification id to a destination |
| `Documents` | List; read; comments |
| `Appearance` | The brand and theme declarations as plain data |
| `Diagnostics` | Version, linked core version, a redacted state dump for support |
| Client state machines | One per feature slice (section 7): the domain state each surface draws from, and its actions |

```rust
#[derive(uniffi::Object)]
pub struct Session { /* … */ }

#[uniffi::export(async_runtime = "tokio")]
impl Session {
    pub async fn runs(&self, filter: RunFilter) -> Result<Vec<RunSummary>, MobileError>;
    pub async fn decide(&self, run: RunId, request: ApprovalId, decision: Decision)
        -> Result<(), MobileError>;
    pub fn watch_run(&self, run: RunId, listener: Arc<dyn RunListener>) -> Subscription;
}

#[uniffi::export(with_foreign)]
pub trait RunListener: Send + Sync {
    fn event(&self, event: RunEvent);
    fn closed(&self, reason: StreamEnd);
}
```

## 4. Conventions across the boundary

| Concern | Rule |
|---|---|
| Async | Rust `async fn` is exported as Swift `async` and a Kotlin `suspend` function, on one runtime owned by the core |
| Streams | A listener interface implemented on the native side, called from Rust; a `Subscription` handle cancels on drop. The core resumes a dropped stream from its cursor; the native side never tracks cursors |
| Errors | One `MobileError` enum with typed variants (not signed in, network, refused, not found, version too old, internal). No string errors; an internal error carries a correlation id, not a cause chain |
| Identifiers | The server's typed ids, exported as distinct types |
| Time | Timestamps as instants; formatting is the native side's |
| Threads | Listener callbacks arrive off the main thread; the generated API documents it and each app hops to its UI thread in one place |
| Lifecycle | `foreground()` and `background()` are called by the app; the core opens and closes streams accordingly |
| Secrets | Tokens, the sealed credential and signing keys never cross the boundary. The native side holds no secret and cannot ask for one |
| Versioning | The binding surface carries the core version; the app refuses to start on a mismatched library |

## 5. Platform modules

`systemprompt-device` gains two implementations of its platform contract.

| Concern | iOS | Android |
|---|---|---|
| Sealing the credential | Keychain, device-only, not synced to cloud backup | Keystore-wrapped key; ciphertext in app-private storage; excluded from backup |
| Biometric gate (optional, by policy) | The app requests it before the core unseals | Same |
| Files | The app's container | App-private storage |
| Local store | Bundled SQLite in the container | Same in app-private storage |
| Cryptography provider | Installed at start-up for the HTTP client | Same |

Anything that needs a system prompt (a biometric sheet, the passkey sheet, the browser sheet for sign-in) is presented by the app; the core supplies the request and receives the result.

## 6. The phone composition

The phone is a composition in the harness's sense, built by `systemprompt-mobile` from the `device` and `client` slices only (`02-target-architecture.md` section 5). Members contribute logic exactly as on the desktop: a settings block, a session observer, a notification category, a projection of a manifest entry. They cannot contribute a screen: screens are native code (`05-native-apps.md` section 5).

## 7. Shared client state machines

Each feature slice provides its state machine in core, in the `client` part of `systemprompt-harness`. The desktop uses it directly; Swift and Kotlin use it through the bindings.

| State machine | Owns |
|---|---|
| Session stream | Opening a session from snapshot; applying ordered events and state deltas; resuming from a sequence number; the approval-wait and offline states |
| Approval queue | Pending requests in order; one idempotent decision; the outcome when another client decided first |
| Inbox | The feed, unread count, and resolving a notification id to a destination |
| Documents | Paging and the comment thread |
| Sign-in | The three sign-in paths, key trust, sync state |
| Preferences | The user's own settings |

Conventions:

- A state machine exposes the current domain state and a change callback, and accepts actions. It owns its streams.
- It reports loading, empty, error, stale and waiting explicitly.
- It does **not** decide what a screen shows: no grouping, no formatting, no field selection, no layout. Each surface shapes its own screens from the domain state. This is what keeps a fork free to redraw a screen without a core change.

## 7a. Who owns the binding library

The final library that iOS and Android link is built by a crate in the extension (`extensions/mobile/core`), not by core. The binding generator, in its library mode, generates bindings for every annotated crate linked into one library. So:

- `systemprompt-mobile` in core is an ordinary Rust crate that annotates the shared surface.
- The extension's crate depends on it, adds the fork's own annotated members, and builds the single static or shared library.
- A fork can therefore expose its own Rust logic to Swift and Kotlin without changing core.

## 8. What stays out of the shared core

- Layout, navigation transitions, images, haptics, accessibility labels.
- Push registration with the operating system (the app obtains the token and hands it to the core).
- System sheets: browser sign-in, passkeys, biometrics, the scanner.
