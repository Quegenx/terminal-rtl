# Architecture

Terminal RTL runs the installed agent in a child PTY. The parser retains logical
text; the renderer produces disposable visual rows for the host terminal.

## Directory ownership

```text
bin/                     npm executable entry points
  lib/                   shared native-agent launcher
src/
  main.rs                engine arguments, diagnostics, and demo
  lib.rs                 stable public library entry points
  session/               child and host terminal lifecycle
    mod.rs               resource ownership and cleanup order
    command.rs           child environment and Windows executable resolution
    engine.rs            output/input scheduling and exit decisions
    host.rs              terminal modes, decorations, and resize
    interaction.rs       prefix keys, browsing, paste, and mouse routing
    output.rs            bounded child-output reader
    shutdown.rs          platform termination notifications
  display/
    mod.rs               public display types and exports
    row.rs               bidi reordering and coordinate mapping
    style.rs             cell style and ANSI color serialization
    renderer.rs          redraw, history delivery, and replay
    formatting.rs        conservative Markdown and speaker-label detection
  terminal/
    input.rs             child keyboard, paste, and mouse encoding
    protocol.rs          replies to logical terminal queries and OSC validation
tests/
  display/               rendering and formatting regressions
  terminal/              input encoding and protocol regressions
  regression/            cross-component audit reproductions
  pty/                   real PTY harness, synthetic children, and journeys
  launchers/             Node/Bun entry-point and shim tests
  packaging/             invalid package artifact fixtures
  shell/                 source-integration tests
  fixtures/              shared deterministic data
scripts/
  checks/                test discovery and repository structure checks
  packaging/             binary checks, notices, and extracted-package smoke tests
shell/                   optional source-installation shell integrations
examples/                CPU-only benchmark
vendor/                  patched upstream terminal parsers and their licenses
docs/                    contributor, architecture, release, and audit documentation
```

The root keeps standard discovery/configuration files: `README.md`, `AGENTS.md`,
`LICENSE`, package/Cargo manifests and locks, generated third-party notices, and
editor/git configuration. `.github/workflows/` owns CI.

## Boundaries and compatibility

`session/mod.rs` owns child startup and restores the host terminal before replay
or error propagation. `engine.rs` coordinates the session. Input interpretation
returns an action; it does not spawn processes or render frames. The output
reader applies a bounded queue before the parser receives bytes.

The display code owns visual transformations, row caching, and host serialization.
The terminal code owns logical input encodings and protocol replies. Shared
hyperlink metadata stays in the patched parser so it follows cells through edits.

The public `terminal_rtl::display`, `::input`, `::protocol`, and `::pretty` paths
remain available. Named exports in `lib.rs` preserve those paths; there is only
one implementation of each module. The three npm executable names also remain
unchanged and use `bin/lib/agent-launcher.mjs`.

Cargo test targets stay `display`, `pretty`, `protocol`, `audit`, and `pty`, with
explicit paths in `Cargo.toml`. PTY case names include their module, for example
`cargo test --test pty launchers::interactive_npm_launcher_uses_wrapper_and_preserves_exit_status -- --exact`.
The root `fixture` test is the stable synthetic-child entry point. Node test
discovery recursively finds `.test.mjs` files under `tests/`, including on Node 18.

## File-size policy

First-party source, configuration, and Markdown files must stay at or below
**300 physical lines**, including blank lines and comments. Split by responsibility;
do not compress unrelated logic onto one line to satisfy the limit.

`node scripts/checks/structure.mjs` enforces the limit locally and in CI. Exceptions
are explicit: vendored upstream code, generated third-party notices, lockfiles,
and ignored build/install artifacts (`target/`, `native/`, `dist/`, `node_modules/`).
They retain the layouts required for upstream maintenance, package tooling, and
license distribution. Root documentation entry points remain subject to the limit.

Keep implementation and tests under their owning domain. Update Rust module paths,
Cargo test paths, JavaScript imports, CI commands, and documentation links together
when moving files. Old implementation paths should be removed in the same change.
## Architecture and performance

```text
logical keyboard input -> child PTY -> vt100/vte logical screen
                                      -> bidi display rows -> host terminal
```

Only disposable display rows are reordered. History delivery is separate from
retention, with synchronous backpressure before the next input byte is parsed.
At most one terminal operation's scrolled rows are pending (one screenful).
The output reader also has a bounded queue of approximately 512 KiB.

```sh
cargo run --release --locked --example benchmark -- 200
```

Local Apple Silicon measurement, 2026-09-07: 0.207 ms per full 24×100 frame.
Browsing 10,000 hyperlink-bearing history rows took 3.210 ms with a whole-screen
clone versus 0.194 ms with the new viewport read. These CPU-only measurements
exclude terminal I/O and agent latency.

A separate 24×80 synthetic PTY probe measured 0.010 s wrapper CPU over three idle
seconds (0.33% of one core) and 17.35 ms mean / 19.72 ms maximum visible echo
latency over 20 keys. The 4 ms poll interval remains unchanged; this sample does
not justify replacing the event architecture. No Rust cache, library/CLI enum
split, theme negotiation, branding changes, or extra packaged platforms are
introduced without a demonstrated need.


## Parser maintenance and dependencies

Local patches are documented in [vt100](../vendor/vt100/PATCHES.md) and
[vte](../vendor/vte/PATCHES.md). Keep the hyperlink module and both parser patches
in any proposed release commit. Avoid unrelated vendor-wide formatting.
Upstream vt100 dev dependencies are retained for restoring upstream tests.

Base64 is Windows-only. Version 0.22.1 remains locked: no upgrade is justified
solely by age. Both locked syn major versions serve dependencies and are kept.
The current graph builds with Rust 1.88 on macOS and cross-checks for Windows GNU.

Generate notices reproducibly after dependency changes, then review the diff:

```sh
node scripts/packaging/notices.mjs --write
node scripts/packaging/notices.mjs
```

The inventory includes all 76 locked dependency packages, including target-only
packages. Published winapi GNU import-library crates omit license files; they
declare the same upstream repository and dual license as winapi, whose texts
are included with an explicit attribution. These GNU import libraries are not
linked into MSVC artifacts. Tracked notices remain in every npm package, and
local packing verifies them against Cargo metadata.

CI installs `cargo-audit 0.22.2 --locked` and checks RustSec's current database.
Review every advisory; prefer a focused update or reviewed patch. Do not add a
blanket ignore. Version scans do not certify locally modified parser code. The
local 2026-09-07 run found zero vulnerabilities and no warnings against database
commit `5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5` (1,239 advisories).

