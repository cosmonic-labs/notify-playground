# notify-playground

A hands-on tour of **`cosmonic:notify`** — the interface a WebAssembly component
uses to put a notification, and a question, in front of the person using the
machine.

It serves one page. Compose any notification the interface can express, fire it,
and watch the answer come back. Every control maps to one field of the WIT
record, and every host call the component makes is in one ~40-line file
([`src/notify.rs`](src/notify.rs)) — the sample is meant to be read as much as
run.

![the playground](docs/playground.png)

---

## Run it

You need [Cosmonic Desktop](https://cosmonic.com/downloads) with the
`settings.notifications` flag on (Settings → Labs), and a Rust toolchain with
the `wasm32-wasip2` target.

```sh
rustup target add wasm32-wasip2
cargo build --target wasm32-wasip2 --release
```

Then push it to Desktop's built-in registry and apply the manifest:

```sh
SOCK="$COSMONIC_STATE_DIR/cosmonicd.sock"     # `cosmonicd paths` prints it

wash oci push --insecure oci.localhost:8200/notify-playground:0.1.0 \
  target/wasm32-wasip2/release/notify_playground.wasm

curl --unix-socket "$SOCK" -X POST -H 'content-type: application/json' \
  --data-binary @<(yq -o=json workload.yaml) \
  http://localhost/v1/workloads
```

Open <http://notify.localhost:8200/>.

To run the published build instead of your own, apply
[`deploy/workload.yaml`](deploy/workload.yaml) — same manifest, pointing at
`ghcr.io/cosmonic-labs/notify-playground`.

---

## What it demonstrates

| Control | WIT field | What to notice |
|---|---|---|
| title / body | `notification.title`, `.body` | an empty title is rejected by the **host**, not the component — validation is not something a component can skip |
| urgency | `notification.urgency` | honoured where the platform has the concept; `capabilities().urgency` tells you whether it will be |
| buttons | `notification.actions` | truncated to `capabilities().max-actions`, and the host logs what it dropped |
| button target | `action.target` | `callback` answers the component; `url` opens a link; `deep-link` opens Desktop at a route |
| inline reply | `notification.input` | a real text box on Windows and macOS; on GNOME the host degrades it to a button with the same id |
| tag | `notification.tag` | reuse it to **replace** your previous notification in place, rather than stacking a second |
| timeout-ms | `notification.timeout-ms` | bounds how long `request` will block |
| **post** | `notifier.post` | returns an id immediately; the answer arrives later via `events.pull` |
| **request** | `notifier.request` | does not return until the user answers — the HTTP request in front of it stays open that whole time |
| **close** | `notifier.close` | withdraws it; the pending response resolves as `expired` |

### The three things worth taking away

**1. You never get zero answers, and never two.** Every notification resolves to
exactly one terminal response — `activated`, `action`, `reply`, `dismissed`, or
`expired`. If the user walks away, you get `expired`. If the daemon shuts down,
you get `expired`. Write your component around that guarantee rather than around
a timeout of your own.

**2. `post` and `request` are the same notification, delivered differently.**
`request` blocks the invocation. `post` returns an id and the answer lands in a
queue you drain with `events.pull`. The queue is per **component**, not per
instance — which is the only thing that works for a request-scoped component
like this one, where the invocation that posted is long gone before anyone
clicks.

**3. Ask what the platform can do; don't assume.** `capabilities()` is cheap and
tells the truth, including the uncomfortable truth that sometimes nothing can be
shown at all — a headless Linux session, macOS authorization declined, a dev
daemon with no bundle identity. Check `available` rather than discovering it one
`unavailable` error at a time.

---

## How do I call back into the Cosmonic product?

Every button (and the notification body) carries an `action-target` saying
what activating it does. Three kinds:

| Target | What the host does | Use it for |
|---|---|---|
| `deep-link` | opens **Cosmonic Desktop** at an app route (`cosmonic://navigate/<route>`) | "show me that workload / setting / lesson" |
| `url` | opens the default **browser** (http/https only) | release notes, dashboards, docs |
| `callback` | nothing host-side | the answer just comes back to your component |

### Deep-link routes the app recognises

| Route | Opens |
|---|---|
| `workloads/<ns>/<name>` | Workloads, with that workload revealed (scrolled to + flashed) |
| `settings` | Settings — e.g. where to send someone after a `denied` |
| `settings-updates` | Settings, opened on the Updates section |
| `logs` | The Logs screen |
| `academy/<category>/<lesson>` | An Academy lesson in the reader |
| `launchpad/<category>/<entry>` | Launchpad, with that entry highlighted |

Anything else — including a route added in a Desktop version your user has
not installed yet — **fails closed to focusing the window**. Two independent
layers each do their half: the daemon validates the route's *shape*
(relative paths only — no scheme, no leading `/`, no `..`, no `?` or `#`;
violations are `notify-error::invalid` before a toast ever shows), and the
app whitelists the route's *meaning*.

### The free callback: the toast body

Set no `default_target` and a body click gets the **host default** — a
deep-link to *your workload's own page* (`workloads/<ns>/<name>`), composed
by the host from your workload identity. You never hard-code your own name;
"open the app on me" is what every notification does out of the box. Send
`"default_target": {"target": "callback"}` if you truly want a body click to
do nothing host-side.

### Example

Against this playground's `/post` route:

```json
{
  "title": "Deploy finished",
  "body": "checkout 0.3.1 is running",
  "actions": [
    { "id": "show",  "label": "Show me",       "target": "deep-link", "value": "workloads/default/checkout" },
    { "id": "notes", "label": "Release notes", "target": "url",       "value": "https://example.com/notes" },
    { "id": "ack",   "label": "Dismiss",       "target": "callback" }
  ]
}
```

Or straight from Rust against the WIT bindings (see `src/notify.rs`):

```rust
Action {
    id: "show".into(),
    label: "Show me".into(),
    target: ActionTarget::DeepLink("workloads/default/checkout".into()),
}
```

The page's **"calling back into cosmonic"** card has a one-click probe for
every route and target kind — including a *refused* group whose invalid
targets demonstrate the validation.

---

## Reading the code

```
src/notify.rs   every host call, and the WIT→JSON conversions. Start here.
src/lib.rs      HTTP routing. Nothing notify-specific.
src/index.html  the page. Explains each control inline.
wit/            the world this component imports, plus the notify package.
workload.yaml   the manifest. One `hostInterfaces` entry is the whole ask.
```

Bindings come from `wit_bindgen::generate!` over `wit/`. The HTTP export is
generated separately by `wasmcloud-component`'s own macro, which is why
`wit/world.wit` declares only the imports.

---

## Seeing responses without a working OS backend

On a machine where notifications cannot actually be displayed — a dev daemon on
macOS, a headless CI runner — start the daemon with the mock backend:

```sh
COSMONIC_NOTIFY_BACKEND=mock cosmonicd run
```

Notifications are then logged instead of shown, and you can answer on the user's
behalf:

```sh
curl --unix-socket "$SOCK" -X POST -H 'content-type: application/json' \
  -d '{"id":1,"kind":"action","actionId":"approve"}' \
  http://localhost/v1/notify/simulate
```

The page builds that command for you. It cannot run it itself, and that is worth
understanding: the daemon's control API is a unix socket with no TCP bind, and a
component has no way to reach it. `cosmonic:notify` is the *only* channel
between this component and the daemon. A component that could call the control
API could run arbitrary workloads and resolve secrets — so simulated responses
come from your terminal, not from the sandbox.

---

## Turning it off

Settings → Notifications lists every workload that has asked to notify you, with
a switch each. Revoking one takes effect immediately, survives a daemon restart,
and is enforced in the daemon — the component's next `post` returns `denied`.

---

## See also

- [`docs/NOTIFY.md`](https://github.com/cosmonic/desktop/blob/main/docs/NOTIFY.md) — the design, the platform matrix, and the constraints behind each decision
- [`cosmonic:notify@0.1.0`](wit/deps/cosmonic-notify/notify.wit) — the interface, commented

Apache-2.0.
