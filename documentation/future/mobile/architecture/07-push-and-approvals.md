# 07. Push notifications and approvals

## 1. Push

### 1.1 Shape

Each instance sends its own push notifications, with its own credentials, for its own app. There is no shared relay.

```
a producer writes a notification ──► outbox ──► pipeline (recipients, rules, quiet hours)
                                                   │
                         ┌─────────────────────────┼─────────────────────────┐
                    in-app channel            Apple push channel        Google push channel
                    (event stream)            (if a live iOS token)     (if a live Android token)
```

The two push channels are implementations of the existing channel trait. Leases, receipts and retries are the pipeline's.

### 1.2 What a push carries

An opaque notification id, a category, and a collapse key. Nothing else.

- No title or body from the server, no run content, no document text, no user name. The app shows a fixed, localised line per category ("An approval is waiting") and fetches detail from the instance after the device is unlocked.
- Reason: push payloads transit and are stored by a third party; governance content must not.
- A category may carry actions (allow, deny). An action taken from the notification calls the instance; it is not decided on the device.

### 1.3 Tokens

| Event | Effect |
|---|---|
| The app obtains a token from the operating system | The core registers it against the calling device's record |
| The token changes | Re-registered; the old one is replaced |
| Sign-out, device revoked, user deleted | Token removed |
| The provider reports a token as unregistered | Token removed in that delivery |
| Notifications disabled in system settings | The app reports it; the instance keeps the token and the pipeline's rules decide |

A caller can write only its own device's token.

### 1.4 Credentials

| Platform | The instance holds | Scope |
|---|---|---|
| Apple | A signing key with its key and team identifiers | The publisher's apps |
| Google | A service account for the publisher's project | The publisher's app |

They are instance secrets in a config plane. Rotating them needs no app release. An instance without them sends no push and loses nothing else.

An install whose app is published by someone else (for example, an organisation using the reference app) needs that publisher's push credentials on its instance, or it runs without push. This is the cost of having no relay, and is stated in the risks.

### 1.5 When the app is open

The in-app channel on the event stream delivers; the pipeline suppresses the push for a recipient with a live foreground stream on that device.

## 2. Approvals

### 2.1 The flow

```
agent asks for a tool the mode does not pre-allow
        │
  run ► awaiting_approval ── event: approval_requested ── notification (high attention)
        │                                                      │
        │                              in-app stream ◄─────────┴────────► push to the phone
        │
  owner decides on any signed-in client (phone, web, desktop)
        │
  first decision wins ── event: approval_decided ── other clients update
        │
  allow: the tool runs          deny: the agent is told, with feedback
        │
  no decision before the timeout: denied
```

### 2.2 Rules

| Rule | Statement |
|---|---|
| Who decides | Only the run's owner |
| Where | Any of the owner's signed-in clients. The decision is made on the server |
| Concurrency | The first decision is recorded; a later one receives the recorded outcome and changes nothing |
| Timeout | Denies. A configured interval per instance; never an implicit allow |
| What the decider sees | The tool, a redacted summary of its input, the agent, the run. Redaction is the server's, by the same rules as audit views |
| Feedback | A deny may carry text returned to the agent |
| Standing answers | "Always allow this tool for this run" is recorded on the run and applies only to it |
| Governance first | The instance's own verdict is evaluated before a person is asked. A person cannot allow what policy denies |
| Hosts that cannot pause | Refused at queue time when an asking mode is requested |
| Audit | Request, decision, decider, client, time: in the run's events and the governance journal |

### 2.3 On the phone

- The home screen lists pending approvals first.
- A push in the approval category offers allow and deny after the device's own unlock; policy may require the app's biometric gate as well.
- Losing connectivity mid-decision is safe: the decision is one idempotent call, and the app shows the recorded outcome when it reconnects.

## 3. Security review

Both parts are reviewed independently before M1 closes: token registration (a user cannot register a token on another's device; a revoked device cannot re-register) and the approval step (owner only; timeout denies; a second decision cannot overturn the first; a person cannot widen policy).
