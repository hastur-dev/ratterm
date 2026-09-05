# Is it better than the baseline?

The work in this branch implements the recommendations in
`docs/improvement_recommendations.md`. This is the evidence for whether that
made the system better, and where it did not.

Baseline is commit `5f90fc3` ("fix: install scripts resolve the actual released
version"), the state of the branch before this work started.

## The headline

Yes, with two qualifications stated in full below: the file-size rule is still
broken in 37 files, and several new subsystems have no test that talks to the
real thing they wrap.

The strongest single piece of evidence is not a count. It is that at baseline
`cargo test` did not finish, and now it does.

At baseline, `cargo fmt --check` and `cargo clippy --all-targets
--all-features -- -D warnings` were both clean, and the test command exited
101:

```
test test_pty_kill has been running for over 60 seconds
test test_pty_large_output has been running for over 60 seconds
test test_pty_read has been running for over 60 seconds
test test_pty_shutdown has been running for over 60 seconds
error: test failed, to rerun pass `--test terminal_pty_tests`
```

Four PTY tests hung. The cause was that `Pty` dropped its `Child` without
killing it: on Windows the ConPTY handle stayed open as long as the child did,
so `drop` blocked. One run measured 198.76 seconds inside `drop(pty)`.

## What could not be done before, and can be now

| Question | Baseline | Now |
|---|---|---|
| Does the test suite pass? | It did not finish. `test_pty_large_output` spent 198.76s inside `drop(pty)` and the run hung. | Full suite green. |
| Do unsaved edits survive a tab switch? | No. Switching tabs re-read the file from disk, discarding the buffer and its undo history. | Yes. Each tab owns its document; switching swaps state. |
| Where are SSH passwords kept? | Base64 in `~/.ratterm/ssh_hosts.toml`, under a "derivation" that ignored both the salt and the password. | OS keychain by default; an Argon2id + XChaCha20-Poly1305 file where there is no keychain; plaintext only if asked for. |
| Can another local process drive the editor and the shell? | Yes. The IPC endpoint had no authentication at all. | No. A per-session token, `0600`, required on the first message. |
| Does an SSH command reuse a connection? | No. Every call spawned `plink`/`ssh` afresh. | A pooled, persistent session per host, with known-hosts verification and ProxyJump. |
| Can an agent see the interface? | No. The API could drive the PTY and the buffer and could not read a single popup, dashboard or status line. | `app.snapshot` renders a frame off-screen and returns it as text or as styled cells; `app.send_key` and `app.send_mouse` drive it. |
| Can it run without a terminal? | No. | `--headless WxH`, and `--scenario` files that assert on frames. |
| Does a metric survive a restart? | No. Two collectors each kept the latest sample in a `HashMap`. | One ingest path, a SQLite history with retention and downsampling, and alert rules evaluated on every sample. |
| Can a Windows or macOS host report metrics? | No. The reporter was a Bash script needing `/proc` and `curl`. | `rat-agent`, built from the same sources, on every platform ratterm builds for. |
| Are containers on several hosts visible at once? | No, one host at a time through the `docker` CLI's text output. | The Engine API through bollard, several hosts at once, one fleet view. |
| Is there any Kubernetes support? | None. | Contexts, five resource kinds, scale, rollout restart, delete, pod logs, port forward. |
| Is a misspelled setting reported? | No. `metrics_hisory = true` left history off and said nothing. | `--check-config` reports it with a line number and a suggestion; start-up says so in the status bar. |
| Does CI cover Linux, Windows and macOS? | Partly. | Nine jobs across ubuntu, windows, macos and arm64, including the scenario suite and a headless smoke test on each. |

## Counted

Measured from git for the baseline and from the working tree for the current
state; the script is reproducible from `docs/improvement_evidence.md` history.

| Measure | Baseline | Now |
|---|---|---|
| Source files | 222 | 372 |
| Source lines | 80,006 | 129,648 |
| `#[test]` attributes | 1,259 | 3,004 |
| Integration test files | 21 | 32 |
| Interface scenarios | 0 | 12 |
| Files over 500 lines | 47 | 53 |
| Files whose *code* exceeds 500 lines | — | 37 |

The test count is 2.4x, and that ratio understates the change: the baseline's
tests were concentrated in the parser, the grid and the host list, while the
new ones cover the parts that had none — credentials, sessions, ingest,
validation, key maps and rendering.

## Where it is not better

**The 500-line rule is still broken, in 37 files.** The project's own
instructions cap a file at 500 lines. The baseline broke it in 47 files; this
branch broke it in 53, of which 37 exceed the limit in code alone rather than
in tests. Some of that is inherited (`completion/keyword.rs` at 1,345 lines of
code, `terminal/mod.rs` at 1,160, both untouched here). Some of it is mine:
`app/mod.rs` grew from 775 to 1,062 and `app/render.rs` from 258 to 943 as
screens were added to them. Two Docker files and two of my own were split; the
orchestrator was not.

**Several new subsystems have no test against the real thing.** Kubernetes
listing, watching, exec, port forwarding and log streaming need a live API
server. Docker's typed client needs a daemon. The SSH session layer needs a
host. In each case the logic underneath is pure and tested, and the wrapper is
deliberately thin — but "the patch body is exactly right" is not the same
claim as "scaling a deployment works", and this branch only establishes the
first.

**The Docker fleet refreshes on the calling thread.** Each host is connected
and listed in turn, so a fleet with several unreachable hosts pauses the
interface for the length of the connect timeouts. `DockerFleet::record_snapshot`
exists to do this on a worker thread; nothing calls it yet.

**Pod log following polls rather than streams.** `kube`'s log stream needs the
`futures-io` traits, and no `futures` dependency was added. The follower asks
for the last few seconds once a second and drops what it has already shown:
the same lines, up to a second later.

**Remote Docker needs a TCP listener.** An SSH `direct-tcpip` channel cannot
reach a Unix socket, so a remote daemon must listen on `127.0.0.1:2375`. Hosts
without it fall back to the CLI path.

**93 `#[ignore]`d tests do not pass when you run them.** They are the
`tests/expectrl_*` suites, which spawn the real binary in a Windows ConPTY.
They were ignored with the reason "requires `cargo build --release` first", and
the harness hardcoded the release path — which is why nobody noticed. The path
now falls back to the debug build, so they can be run with an ordinary build,
and when run they fail:

```
$ cargo test --test expectrl_smoke_tests -- --ignored --test-threads=1
test result: FAILED. 1 passed; 3 failed
  Timed out waiting for 'ratterm v' after 5s. Buffer: 0 bytes, stripped: 0 chars.
```

Zero bytes come back from the ConPTY, while the same binary run directly
prints `ratterm v0.2.2` immediately. The harness is broken on this machine, not
the application. It was left broken: the scenario runner added in this branch
covers the same ground deterministically, on three platforms, without a PTY,
and repairing a Windows-only harness to duplicate that is not obviously worth
doing. Whoever disagrees now has the failure in front of them rather than
behind an `#[ignore]` with a misleading reason.

**`.claude/CLAUDE.md` still says edition 2021 and MSRV 1.75**; the manifest says
2024 and 1.89. That file is gitignored, so it could not be fixed on this
branch — it needs a one-line edit in the working copy.

## What the evidence does not show

Nothing here measures whether the application is *pleasant to use*. The
scenarios assert that a screen opens, contains what it should, and closes;
they say nothing about whether the fleet view is the right shape or whether
anyone wants a Kubernetes client in their terminal. Seven interface defects
were found by writing those scenarios — a command palette that rendered
nothing after visiting the Docker manager, a `Ctrl+T` binding advertised
globally and reachable only when the IDE pane was already visible — which
suggests the interface had more of them than anyone had counted, and that the
twelve scenarios now in the tree are a floor rather than a ceiling.
