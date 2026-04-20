---
id: T02
parent: S04C
milestone: M001
key_files:
  - src/infrastructure/connect/mod.rs
  - src/infrastructure/mod.rs
  - Cargo.toml
key_decisions:
  - tiny_http for the callback server — ~600KB compiled, blocking API fits our synchronous CLI architecture, no tokio drag. webbrowser crate for cross-platform launch with a print-fallback for environments where it can't open (SSH, CI).
  - Random port (127.0.0.1:0) — tiny_http asks the OS to assign. Avoids conflicts if the user runs Teller + Pluggy connect back-to-back, and no fixed port to document.
  - run_until_callback takes the HTML page as a parameter — keeps ConnectServer provider-agnostic. Teller + Pluggy templates live alongside the CLI commands that use them.
  - render_template's `{{KEY}}` syntax — simple enough to not need a real templating engine. Values passed in are always safe (app IDs, connect tokens, UUIDs), no escaping needed.
duration: 
verification_result: passed
completed_at: 2026-04-19T23:56:43.540Z
blocker_discovered: false
---

# T02: Reusable ConnectServer (tiny_http on random localhost port) + webbrowser launcher + render_template helper; /success /failure /exit callback capture with 5-minute timeout. 7 unit tests covering all three callback kinds + GET/POST routing + timeout.

**Reusable ConnectServer (tiny_http on random localhost port) + webbrowser launcher + render_template helper; /success /failure /exit callback capture with 5-minute timeout. 7 unit tests covering all three callback kinds + GET/POST routing + timeout.**

## What Happened

Reusable local HTTP callback server + browser launcher for provider Connect widgets.

`ConnectServer::bind()` binds tiny_http on 127.0.0.1:0 (random free port). `run_until_callback(html, timeout_secs)` serves `GET /` with the provided HTML, 204s on favicon, captures POST on `/success`/`/failure`/`/exit` (returns `CapturedCallback { kind, body }`), 404s everything else. Shuts down on first terminal callback or when the deadline hits.

`launch_browser(url)` uses the `webbrowser` crate with a graceful fallback: if the launch fails (SSH, headless, weird env), prints the URL to stderr so the user can paste it manually. Never errors.

`render_template(template, vars)` — simple `{{KEY}}` substitution. Test verifies unknown vars pass through unchanged.

Deps added: `tiny_http = "0.12"`, `webbrowser = "1"`.

7 unit tests: captures Success/Failure/Exit callbacks; times out when no callback arrives (1s timeout); serves the supplied HTML on `GET /` while keeping the server up; 404s unknown paths without terminating; template substitution happy + unknown-var passthrough. All tests use real HTTP via ureq against the running server, no mocking.

## Verification

`cargo test` all green including the 7 new Connect tests. Server shutdown and port release verified indirectly — adjacent tests don't port-conflict.

## Verification Evidence

| # | Command | Exit Code | Verdict | Duration |
|---|---------|-----------|---------|----------|
| 1 | `cargo test` | 0 | pass | 1180ms |

## Deviations

None.

## Known Issues

None."

## Files Created/Modified

- `src/infrastructure/connect/mod.rs`
- `src/infrastructure/mod.rs`
- `Cargo.toml`
