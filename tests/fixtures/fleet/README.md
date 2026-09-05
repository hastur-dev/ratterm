# Fleet fixtures

Canned state for scripted runs, loaded with `rat --fixtures tests/fixtures/fleet`.

Four hosts, one of them behind a jump host and one deliberately offline, plus
metrics for the health dashboard. The addresses are in the RFC 5737
documentation range and resolve to nothing; on top of that, loading fixtures
clears the shared remote executor, so a run cannot reach a real machine even if
an address did resolve.

| File | Purpose |
|---|---|
| `ssh_hosts.toml` | Hosts and credentials, in the same format as `~/.ratterm/ssh_hosts.toml` |
| `metrics.json` | One entry per host, seeding the health dashboard |
| `docker_items.toml` | Quick-connect slots and the selected Docker host |

Every file is optional.
