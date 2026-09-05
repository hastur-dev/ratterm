# Kubernetes

`Ctrl+Shift+K` opens the Kubernetes screens; pressing it again closes them.
Everything happens over the same connection ratterm already has: a cluster on
the local network is reached directly, and a cluster behind a fleet host is
reached through the SSH forward that host's session already provides.

## Contexts

The first screen lists the contexts from `$KUBECONFIG`, or `~/.kube/config`
when that is not set. Several files in `$KUBECONFIG` are merged, the way
`kubectl` merges them.

The selection starts on the kubeconfig's `current-context` — what `kubectl`
would use — because landing anywhere else invites connecting to the wrong
cluster.

A context whose cluster the file does not define is listed in red rather than
hidden. Seeing it, and seeing that its server is `<cluster not defined>`, is
what tells you which entry to fix.

| Key | Action |
|---|---|
| `Up` / `Down`, `k` / `j` | Select |
| `Home` / `End`, `g` / `G` | First / last |
| `Enter` | Connect |
| `Esc` | Close |

Connecting builds a client and lists the cluster's namespaces. No request is
sent while the client is built, so a cluster that is configured but down
connects and then reports itself unreachable on the first listing — which is
also the first thing the resource screen does.

## Resources

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | Next / previous kind |
| `Up` / `Down`, `k` / `j` | Select |
| `/` | Filter |
| `r` | Refresh |
| `+` / `-` | Scale the selected deployment by one |
| `R` | Restart the selected deployment's rollout |
| `Delete` / `d` | Delete the selected pod |
| `Backspace` | Back to the context list |
| `Esc` | Close |

Five kinds are listed: pods, deployments, services, nodes and events. Nodes are
cluster-scoped, so the namespace filter does not apply to them and is not shown
while they are listing — a node list "in" a namespace would simply be empty.

A row is drawn in red when it describes something unhealthy. What that means is
per kind and is decided in one place:

- a pod whose containers are not all ready, or whose status names a back-off
  or an error — a pod stuck in `CrashLoopBackOff` reports `1/1` at some points
  in the loop, so the count alone is not enough;
- a deployment with fewer ready replicas than it wants, or a paused rollout. A
  deployment scaled deliberately to zero is not unhealthy;
- a node that is not `Ready`;
- an event whose type is not `Normal`.

### Filtering

`/` starts a filter. It matches a substring of any column, ignoring case, which
is what finding `checkout` inside `checkout-7d9f8b-x2k4p` needs. While the
filter box is open every printable key is text, so a filter containing `r` or
`d` can be typed; `Esc` clears it and `Enter` closes the box while keeping it.

### Scaling and restarting

Scaling moves by one replica per key press rather than reading a typed number.
A prompt over a cluster action is a thing that gets confirmed by accident, and
`+` held down is a clear enough way to reach ten.

Deleting a pod is offered because it is how a rollout gets nudged — the
controller replaces it. Deleting a deployment, which is not recoverable that
way, is deliberately not offered here.

## Reaching a cluster through a fleet host

A cluster on a private network is reached through the SSH session ratterm
already keeps to that host: the API server port is forwarded to a loopback port
and the client is pointed at it, with the certificate's server name preserved so
TLS still verifies. The forward lives as long as the connection and is closed
when the screens are.

`~/.ratterm/k8s.toml` remembers pinned contexts, favourite namespaces per
context, and which context was last used.

## Scripted runs

With `--fixtures <dir>`, the screens read `<dir>/kubeconfig` and never
`~/.kube/config`, so a scripted run's output does not depend on whose machine
it ran on. A fixture directory with no kubeconfig produces a screen saying so.

`tests/fixtures/fleet/kubeconfig` ships three contexts covering an ordinary
context, a current context with a non-default namespace, and one whose cluster
is undefined. Its servers are in the RFC 5737 documentation range, which routes
nowhere, so a connection attempt cannot reach a real cluster.

Under `--test-keys`, and in scenarios, `F6` opens the screens. Not `F5`: that
is the debugger's "start".

## What is tested, and what cannot be

Everything under the screens is pure and tested without a cluster: kubeconfig
parsing and merging, the context list, every conversion from an API object to a
row, the health rules above, the filter, the sort comparators, the exact scale
and restart patch bodies, the column layout, and the key map.

What needs a live API server — listing, watching, exec, port forwarding and log
streaming — is not covered by an automated test here. Those functions are
deliberately thin wrappers over `kube`, with the logic beneath them tested
directly.
