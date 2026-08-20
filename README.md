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
