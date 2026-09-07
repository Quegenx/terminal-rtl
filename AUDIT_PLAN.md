# Terminal RTL — consolidated audit remediation plan

Date: 2026-09-07  
Scope: current working tree, including the uncommitted 0.1.5 / OSC 8 changes.  
Status: planning only; implementation, deletion, staging, committing, and publication are not authorized by this document.

## Objective

Make terminal geometry, process cleanup, escape parsing, history, hyperlinks, and platform launch behavior reliable before the next release. Preserve logical child text, existing CLI argument semantics, and the documented display-only design.

This plan reconciles the Codex audit with the supplied Grok and Claude Code audits. Priority follows reproduced behavior and user impact, not the number of findings or whether the existing test suite passes. Claude's additional CI evidence was independently checked through read-only GitHub queries on 2026-09-07.

## Evidence and reconciliation

Evidence labels:

- **Reproduced:** exercised during the preceding Codex audit, outside the repository.
- **Confirmed:** supported by current repository code or configuration.
- **Verify:** plausible concern that still needs an end-to-end or platform-specific test.
- **Decision:** intentional behavior or product policy requiring an explicit choice before implementation.

| Audit claim | Consolidated disposition |
| --- | --- |
| Retained history may misalign after resize | Stronger evidence exists: widening history crashes rendering; reproduced in debug and release. Prioritize P01. |
| Only the untracked file is High | Do not adopt this assessment. Four geometry crash scenarios, unbounded OSC accumulation, and skipped SIGTERM cleanup warrant earlier runtime fixes. |
| `hyperlink.rs` is untracked | Confirmed release-completeness prerequisite. The working tree builds; a future commit fails only if references are committed without the new module. Include the file when committing, rather than describing the current checkout as broken. |
| Native-launcher test is TTY-sensitive | Reject as stated. `spawnSync` defaults to piped stdio; the test also reads captured stdout. Running the test from a TTY does not make its spawned launcher interactive. Add separate interactive coverage. |
| Bun and Node test runs are redundant | Retain both: both runtimes are supported. Reusing a freshly captured PTY transcript is an optional efficiency change, not a correctness fix. |
| Bracketed-paste closer injection is confirmed | The helper preserves embedded markers, but runtime exploitability is not established. Crossterm parses paste framing before `paste_bytes` is called. Test the complete host → crossterm → wrapper → child path before claiming a vulnerability or selecting a fix. |
| OSC 8 is safely bounded | Only stored metadata is bounded. Upstream `vte` accumulates OSC bytes before the callback's length check. Fix both resource bounds and parameter truncation. |
| Inline `CSI 3 J` can erase earlier shell history | Confirmed forwarding of a host-wide history-clear sequence. Add P07; actual Codex/Grok emission remains unverified. |
| Set `rust-version = "1.85"` because of edition 2024 | Do not assume the edition minimum is the project's minimum. Check let-chain syntax, standard-library APIs, and locked dependencies, then test the selected version. |
| Root `rtl` fallback is dead | Not produced by the current npm layout, but external/manual layouts may use it. Removal remains conditional. |
| Missing winapi-target notice headings prove missing licenses | Verify applicable targets and existing license coverage first. Missing package headings alone do not establish missing license text or a compliance defect. |
| Moving `session` into the library is necessary for tests | Not necessary: binary modules can contain unit tests. Extract pure helpers only where that improves testing or structure. |
| CI artifact path flattening is broken | Unverified. Inspect the assembled artifact before changing the workflow layout. |
| Windows CI has always failed | All five currently listed `main` runs failed in their Windows job. The latest run has seven PTY failures showing only `ESC[6n`. Four runs contain a skipped package job; the earliest has no package job. This is confirmed evidence for those runs, not a claim about deleted or unlisted history. Add immediate blocker P00A. |
| Answer DSR only in `Harness::until` | Incomplete fix: tests that call `finish` directly also need replies. Use one incremental receive/parser path for both methods, with fragmented-sequence handling and exactly-once responses. |
| The ConPTY diagnosis proves runtime Windows behavior is fine | It explains the observed harness startup stall, but does not certify nested runtime startup, shutdown, or agent launch behavior. Retest them after unblocking CI. |
| `total < mirrored_history` is unreachable | Reject. `clear_scrollback` preserves the counter, but RIS replaces `Screen` and creates a fresh grid with a zero counter (`vendor/vt100/src/screen.rs:1025–1026`). Keep or replace this branch with a better reset-generation mechanism under P07. |
| `if !is_prefix` on `r`/`q` can be deleted | Conditional on key encoding: plain `r`/`q` and modifiers do not encode Ctrl+], but keep literal-prefix regression coverage when simplifying. This is a minor cleanup, not a release blocker. |
| Double spaces can reverse the order of prose fields | The split heuristic is confirmed; Claude's rendered examples do not preserve an unambiguous count of spaces. Add exact-byte one/two/three-space fixtures and compare prose with genuine TUI columns before changing the threshold. See P15. |
| `detect` should run only when pretty is enabled | Already implemented in `Renderer::render` and `append_history`; no change needed. The missing early return in `speaker_labels` remains valid. |
| Broken package CI disproves bundled platform support | No. Local archives/native binaries can provide bundled platforms independently of CI. Document artifact provenance accurately; do not infer npm publication provenance from failed CI alone. |
| Remove Bun, labels, attribution, or shell helpers | These are supported workflows/product features, not dead code. Keep unless an intentional scope/support change is approved. Optional simplifications are separate from remediation. |
| Remove THIRD_PARTY_LICENSES from git | Only consider after an equivalent reproducible generator and validation work for both local and CI packages. Do not remove required notices from distributions. |
| Linux packaging is free because CI builds Linux | It is a promising extension, but requires libc/runtime compatibility, archive, installation, and launcher tests. Keep it optional and outside the correctness release gate. |

## Baseline

Results from the preceding Codex audit; not a fresh implementation validation run:

- 40 Rust tests passed, including one fixture entry point.
- Root rustfmt, all-target Clippy with warnings denied, and release build passed.
- Two JavaScript tests passed under Node 24.20.0 and Bun 1.4.2.
- zsh syntax and `git diff --check` passed.
- OSV returned no matches across 77 package/version queries, including upstream vt100. This does not certify the locally patched parser.
- Default formatting of the vendor crate failed because retained upstream formatting differs; avoid unrelated vendor-wide reformatting.
- Four geometry probes panicked in both debug and release.
- Windows/ConPTY, Node 18, live agent conversations, and remote Actions artifacts were not runtime-verified.

Additional read-only verification after Claude's audit:

- The five listed `main` runs are `34052264499`, `34051479357`, `34049309152`, `34046772219`, and `34045997364`; Windows failed and macOS/Linux passed in each.
- [Latest inspected run](https://github.com/Quegenx/terminal-rtl/actions/runs/34052264499), commit `cf0c5d372f09ab1d86cb123a7f5e62547120a1f4`: seven PTY tests timed out with only `\u{1b}[6n`; the package job was skipped. These remote results predate the current uncommitted 0.1.5 work and its additional tests.
- The installed `portable-pty` 0.9.0 implementation enables `PSEUDOCONSOLE_INHERIT_CURSOR`. [Microsoft documents](https://learn.microsoft.com/en-us/windows/console/createpseudoconsole) that the host must asynchronously answer the resulting cursor query or pseudoconsole operations may hang. This strongly supports the harness diagnosis; the proposed fix still requires a Windows run.
- The latest failed log explicitly warns that `actions/checkout@v4` targets Node 20 and is being forced onto Node 24. [GitHub's migration notice](https://github.blog/changelog/2025-09-19-deprecation-of-node-20-on-github-actions-runners/) supports reviewing all configured action versions. The warning is separate from the demonstrated PTY failure.
- [Rust 1.88 release notes](https://blog.rust-lang.org/2025/06/26/Rust-1.88.0/) confirm stabilization of the let-chain syntax used here. Rust 1.88 is therefore a language-feature lower bound, not yet a verified minimum for the entire locked dependency graph.

Line references below identify the audited snapshot and will move during implementation.

## Phase 0 — preserve a complete, reviewable change

### P00 — release source completeness

- [ ] Preserve all existing user changes; do not reset or overwrite the working tree.
- [ ] When preparing the eventual commit, include `vendor/vt100/src/hyperlink.rs` together with its references in the parser, renderer, and tests.
- [ ] Verify package/Cargo versions and patch notes agree with the final implementation.
- [ ] Validate the eventual commit from a fresh checkout before release.

**Evidence:** confirmed, `vendor/vt100/src/lib.rs:54,64` and untracked `vendor/vt100/src/hyperlink.rs`.  
**Acceptance:** a checkout of the proposed release commit builds and tests without relying on untracked files or pre-existing binaries.  
**Safety:** source inclusion is safe when committing is authorized; staging or committing is not part of this planning task.

### P00A — High: unblock Windows PTY tests and package CI

- [ ] Add host cursor-query responses to the outer PTY harness through a shared output-processing path used by both `until` and `finish`.
- [ ] Parse incrementally across chunks and reply exactly once per query. Do not repeatedly scan the entire captured transcript and resend old replies.
- [ ] For the initial cursor-inheritance handshake, use the known initial host cursor position. If later queries are supported, track the host cursor instead of always answering `1;1`.
- [ ] Keep the outer ConPTY host response separate from the wrapper's logical child-query response; test nested startup explicitly.
- [ ] Run the full Windows PTY suite after the fix and investigate any newly exposed failures; do not assume this handshake is the only Windows issue.
- [ ] Require successful Windows/macOS/Linux jobs and a generated, verified `npm-package` artifact before describing CI packaging as working.
- [ ] Review checkout/upload/download action versions against current official release notes and supported runner requirements; update/pin reviewed releases without conflating deprecation warnings with test failures.

**Evidence:** remote job results and latest failure logs independently verified; `tests/pty.rs:65–104`, `.github/workflows/ci.yml:25–70`, installed `portable-pty` ConPTY creation flags.  
**Acceptance:** the full Windows suite progresses past `ESC[6n`, tests that only use `finish` pass, the package job runs, and its archive passes P11.  
**Safety:** conditional on native Windows CI. Reading logs was authorized; this plan does not trigger workflows, push changes, or publish artifacts.

## Phase 1 — runtime reliability and resource safety

### P01 — High: make historical rows safe across width changes

- [ ] Add regressions for history created at width eight, widened to twelve, then browsed or replayed.
- [ ] Define padding/clipping or reflow semantics for historical rows independently of current viewport width.
- [ ] Remove unchecked cell assumptions from history rendering and final replay.
- [ ] Construct final replay only when replay is enabled and the session is non-inline.
- [ ] Cover normal replay, `--no-replay`, inline mode, and browsing after both widening and shrinking.

**Evidence:** reproduced; `src/display.rs:387–391`, `src/session.rs:533–546`, `vendor/vt100/src/grid.rs:68–83`.  
**Acceptance:** no panic or missing-cell access in debug/release; preserved content follows a documented resize policy. Disabling replay avoids constructing a discarded snapshot.  
**Safety:** conditional on regression coverage; a blanket `unwrap_or` without defining row geometry is insufficient.

### P02 — High: preserve wide-cell and minimum-dimension invariants

- [ ] Add the three reproduced cases: `ab界` shrunk from four to three columns and erased; a wide glyph in one column; wrapping in a one-row/two-column grid.
- [ ] Repair orphaned wide leading/continuation cells during resize.
- [ ] Handle single-row wrapping without underflow or invalid row lookup.
- [ ] Define behavior when the label margin/footer leaves insufficient child space; dynamically reduce decorations or provide a graceful minimum-size state.
- [ ] Consolidate poll-driven and event-driven resizing into one state transition.

**Evidence:** reproduced; `vendor/vt100/src/row.rs:74–95`, `vendor/vt100/src/screen.rs:755–770,910–965`, `vendor/vt100/src/grid.rs:691–703`, `src/session.rs:111–115,199–204,283–292,460–467`.  
**Acceptance:** all cases pass in debug/release and a real PTY; no out-of-bounds host cursor or decoration writes at the minimum supported size.  
**Safety:** conditional; preserve wide-cell behavior during erase, overwrite, insertion, and deletion as well as resize.

### P03 — High: bound OSC parsing before dispatch

- [ ] Enforce a limit while accumulating OSC bytes, not only in `Protocol::unhandled_osc`.
- [ ] Discard oversized sequences through their terminator and resume normal parsing.
- [ ] Ensure bounded parser memory across fragmented and unterminated sequences.
- [ ] Preserve complete supported OSC 8 destinations; never turn a truncated sequence into an accepted link.
- [ ] Document any new dependency patch or parser-boundary adapter in `vendor/vt100/PATCHES.md`.

**Evidence:** source-confirmed and probed; `vendor/vt100/src/parser.rs:4,36,48–49`, `vendor/vt100/Cargo.toml:54–55`, `src/protocol.rs:84–90`. Eight MiB of unterminated OSC was accepted before validation.  
**Acceptance:** bounded allocation is demonstrated without exhausting the test host; trailing ordinary text parses correctly after an oversized sequence terminates.  
**Safety:** conditional; coordinate with P06 so resource limits do not corrupt destinations.

### P04 — High: clean up on catchable termination

- [ ] Route Unix SIGTERM and applicable shutdown signals into normal event-loop cleanup.
- [ ] Define child termination, output-drain timeout, and exit-status behavior.
- [ ] Investigate late ConPTY pipe errors after child exit. Preserve child status for verified normal end-of-stream conditions, but retain genuine I/O errors. Account for a child that has exited before `try_wait` has updated `exit`; blindly ignoring every error when `exit.is_some()` is not a complete policy. (`src/session.rs:317,382–395`.)
- [ ] Define noninteractive JavaScript signal-exit semantics and test self-signaled children. On Unix, consider propagating the signal or its conventional exit status rather than mapping every signal to `1`; define Windows behavior separately. (`bin/native-launchers.mjs:44`.)
- [ ] Verify launcher-forwarded SIGTERM follows that path.
- [ ] Implement and test corresponding Windows console-control behavior on Windows.
- [ ] Keep terminal restoration outside signal-handler-unsafe operations.

**Evidence:** real PTY reproduction; `src/session.rs:65–89,251–252`, `bin/native-launchers.mjs:42–45`. SIGTERM exited without alternate-screen/autowrap restoration.  
**Acceptance:** terminal modes restore after normal exit, catchable termination, startup failure, and panic. Tests must not leave child processes behind. Document unavoidable hard-kill limits separately.  
**Safety:** conditional on lifecycle tests; do not promise cleanup after SIGKILL or equivalent forced OS termination.

### P05 — High, native verification required: correct Windows executable resolution

- [ ] Reproduce with real npm-installed shims on Windows: extensionless shell shim, `.cmd`, and `.ps1` beside one another.
- [ ] Resolve bare command names using compatible executable rules, including PATHEXT ordering and supported file types.
- [ ] Retain explicit executable overrides and direct argument passing for native executables.
- [ ] Align Rust and JavaScript fallback PATHEXT lists.
- [ ] Add shared argument/resolution fixtures covering apostrophes, spaces, empty arguments, Unicode, and metacharacters.
- [ ] Test both interactive Rust wrapping and noninteractive JavaScript batch execution.

**Evidence:** resolver selection reproduced with a controlled JavaScript probe; `bin/native-launchers.mjs:7–12,34–40`, `src/session.rs:137–190`. Native Windows failure remains unverified.  
**Acceptance:** ordinary npm agent installs select an executable Windows path and preserve arguments, cwd, and exit status through both launch modes.  
**Safety:** requires native Windows verification; do not replace direct native execution with blanket shell execution.

## Phase 2 — terminal fidelity and history guarantees

### P06 — Medium: preserve complete hyperlinks through parsing and replay

- [ ] Fix loss of URI segments after `vte`'s OSC parameter limit. Test URLs below, at, and above that boundary.
- [ ] Reuse a validated hyperlink-aware serializer for non-inline exit replay.
- [ ] Close links before labels, footer, erased areas, and subsequent unlinked text.
- [ ] Retain the documented plain/original-text replay policy for Markdown unless intentionally changed; missing pretty formatting is not independently a bug under the current README.
- [ ] Update hyperlink guarantees to match validated live, inline-history, and exit-replay behavior.

**Evidence:** truncation reproduced in debug/release; replay omission source-confirmed. `src/protocol.rs:85–108`, `src/display.rs:296–334`, `src/session.rs:544–559`, `README.md:190–197`.  
**Acceptance:** independent host parsing recovers the exact original destination after wrapping, redrawing, scrolling, and exit replay. Invalid/oversized targets are rejected completely.  
**Safety:** conditional; depends on P03's parsing boundary.

### P07 — Medium: separate native-history delivery from retention and clearing policy

- [ ] Prevent the history retention ring from evicting output before it is mirrored into host scrollback.
- [ ] Use bounded pending delivery/backpressure or drain incrementally; do not introduce an unbounded mirror queue.
- [ ] Test bursts larger than retention capacity and document `--scrollback 0` semantics.
- [ ] Test RIS/history resets when the new counter is smaller than, equal to, or greater than the previous mirrored counter between frames; use a reset generation if needed.
- [ ] Add an xterm test with earlier shell output followed by child `CSI 3 J`.
- [ ] Decide and document whether child history clearing may erase pre-session host history. Recommended default: preserve pre-session history and clear wrapper-owned retained state only.

**Evidence:** eviction before delivery and `CSI 3 J` forwarding confirmed; counter-reset edge cases require additional probes. `src/session.rs:295–320,339–356`, `vendor/vt100/src/grid.rs:192–203,577–587`, `tests/scrollback-host.test.mjs:23–34`.  
**Acceptance:** bursts meet the documented history guarantee, memory stays bounded, resets do not skip/duplicate new rows, and the chosen clear policy is tested.  
**Safety:** conditional/product decision. Standard `CSI 3 J` clears all host saved lines; do not promise selective deletion of earlier mirrored rows without a host capability that supports it.

### P08 — Medium: make terminal modes and resets internally consistent

- [ ] Implement child wraparound mode (`CSI ? 7 h/l`) independently of the host's rendering mode.
- [ ] Return origin-relative cursor coordinates when origin mode is active.
- [ ] Reset callback-owned focus and synchronized-output state on RIS together with parser state.
- [ ] Review pending replies, bell state, link identifiers, and history generation during reset; preserve link-ID uniqueness where old host links survive.
- [ ] Compare behavior against an independent terminal implementation.

**Evidence:** reproduced; `vendor/vt100/src/screen.rs:1024–1026,1166–1239`, `src/protocol.rs:38–59`, `src/session.rs:326–334,470–471`.  
**Acceptance:** disabled wrapping stays on the row; an origin-relative home query returns `1;1`; reset clears negotiated callback modes without stale traffic or indefinite rendering suppression.  
**Safety:** conditional on protocol and PTY regression tests.

### P09 — Verify: bracketed-paste boundary safety

- [ ] Exercise literal opener/closer markers in a synthetic paste through actual crossterm parsing, with varied chunk boundaries.
- [ ] Verify what reaches `Event::Paste` and what becomes subsequent key events on each supported input backend.
- [ ] Define a safe policy for control sequences inside paste while preserving ordinary Hebrew, Unicode, and multiline content.
- [ ] If filtering is needed, implement it at the boundary where it can prevent the demonstrated breakout; helper-only filtering may be too late.
- [ ] Add direct `paste_bytes` tests for its public API contract as well as PTY coverage.

**Evidence:** helper preserves markers; runtime exploitability unverified. `src/input.rs:92–98`, `src/session.rs:455–458`, `tests/protocol.rs` paste tests.  
**Acceptance:** either a reproducible flaw is fixed end to end or the finding is downgraded with documented evidence. Do not label arbitrary stripping as behavior-preserving.  
**Safety:** requires verification; changing pasted bytes can violate the existing exact-text promise.

### P10 — Low/product decision: titles and prefix interaction

- [ ] Decide whether child window/icon titles should be forwarded. Their current omission is confirmed, but it is a capability gap rather than a demonstrated security bug.
- [ ] If enabled, bound and sanitize title payloads and define cleanup/restoration behavior; test OSC 0/1/2 independently.
- [ ] Decide whether the Ctrl+] prefix needs a visible pending indicator, cancellation, or timeout. Its current persistent-prefix behavior is documented key-chord behavior, not inherently incorrect.
- [ ] Preserve the literal Ctrl+] escape and avoid losing input if timeout behavior is introduced.

**Locations:** `vendor/vt100/src/perform.rs` OSC dispatch, `vendor/vt100/src/callbacks.rs` title callbacks, `src/protocol.rs`, `src/session.rs:407–432`.  
**Acceptance:** selected behavior is documented and tested without forwarding raw untrusted escape bytes.  
**Safety:** conditional/product decision; do not silently change user-visible interaction during unrelated bug fixes.

### P15 — Medium, behavior verification required: distinguish prose gaps from TUI columns

- [ ] Add exact UTF-8 fixtures with one, two, and three ASCII spaces between Hebrew words/sentences; store logical input and expected visual order explicitly so Markdown rendering cannot collapse evidence.
- [ ] Include mixed Hebrew/English, punctuation, indentation, tables, and genuinely independent TUI columns.
- [ ] Compare against Unicode bidi behavior for a single prose field, then identify where the existing documented layout heuristic intentionally differs.
- [ ] If ordinary prose is demonstrably reordered incorrectly, choose a targeted heuristic or explicit layout option. Do not simply change two spaces to three without testing both prose and columns.
- [ ] Keep cursor/mouse mapping, wide cells, and hyperlinks consistent with the selected field boundaries; document remaining ambiguity.

**Evidence:** split at two spaces is confirmed, `src/display.rs:137–205`; Claude's specific prose examples require exact-byte reproduction.  
**Acceptance:** agreed prose fixtures read in the intended order, stable UI columns remain fixed, and coordinate/link tests pass.  
**Safety:** conditional; there is no universally correct way to infer semantic columns from whitespace alone.

## Phase 3 — release validation, dependencies, and documentation

### P11 — Medium: validate the assembled package

- [ ] Strengthen `scripts/check-package.mjs`: regular-file checks, expected format/architecture, executable permissions where applicable, and version matching.
- [ ] Add negative fixtures: text files over 1,000 bytes, wrong architecture, missing binary, non-executable Unix binary, mismatched version.
- [ ] Inspect actual downloaded Actions artifact paths before changing flattening/copy logic.
- [ ] Smoke-test extracted package launchers on macOS and Windows, without a repository `target/` fallback or unrelated installed `rtl` masking missing artifacts.
- [ ] Verify local GNU Windows builds and CI MSVC builds against the intended support policy; select/document which artifacts may ship.
- [ ] Compare Cargo and npm manifest versions automatically; make README packaging examples version-independent or validate their version references.
- [ ] Keep packaging separate from publication.

**Evidence:** current checker accepted two 1,001-byte text files in an isolated probe. `scripts/check-package.mjs:3–8`, `.github/workflows/ci.yml:40–70`, `package.json:27–32`.  
**Acceptance:** malformed/stale artifacts fail validation and the produced archive launches successfully on each bundled platform.  
**Safety:** validation changes are safe; architecture/runtime support changes require platform verification.

### P12 — Medium: expand coverage without deleting useful checks

- [ ] Retain both Node and Bun coverage; test the declared minimum Node version or update the support contract deliberately.
- [ ] Keep the existing piped launcher test and add a separate controlled interactive PTY test.
- [ ] Extend independent xterm checks to geometry, clear/reset, replay, and hyperlinks; avoid validating every terminal behavior with the same parser on both sides.
- [ ] Add Windows-only resolution/encoding tests and shell-helper tests for aliases, existing functions, nested sessions, and missing executables.
- [ ] Fill specific branch gaps: `--direction ltr/rtl`, `--no-bidi`, recording creation/non-overwrite/permissions, `--scrollback 0`, F1–F12 and editing keys, X10/UTF-8 mouse encoding, timestamp boundaries, and unified-launcher help/unknown-agent behavior. Select meaningful cases rather than mirroring every implementation branch.
- [ ] Add sanitized, explicitly sourced/versioned CLI-output fixtures for label/layout heuristics where synthetic cases are inadequate. Do not commit raw private conversations or secrets from recordings.
- [ ] Give all process tests bounded timeouts and deterministic cleanup.
- [ ] Optionally reuse a freshly produced capture between Rust and JS checks, preserving freshness and isolation; do not substitute stale golden output for live integration coverage.

**Locations:** `tests/native-launchers.test.mjs:6–15`, `tests/scrollback-host.test.mjs:9–36`, `tests/pty.rs`, `shell/rtl.zsh`, `shell/rtl.ps1`, `.github/workflows/ci.yml:30–39`.  
**Acceptance:** each Phase 1/2 fix has a focused regression at the appropriate layer; supported runtime/platform branches are explicitly exercised.  
**Safety:** safe with deterministic fixtures; no live prompts, messages, or agent API calls are required.

### P13 — Low: dependency, toolchain, and notice hygiene

- [ ] Move Base64 into Windows target dependencies; retain its Windows use.
- [ ] Evaluate the newer Base64 version independently, rather than treating age as a vulnerability.
- [ ] Remove unused vendor dev-dependency declarations only if upstream tests will not be restored; otherwise retain/document their purpose.
- [ ] Test Rust 1.88 as the language-feature lower bound against the current locked dependency graph, then declare the actual supported `rust-version`; raise it if dependencies/APIs require more. Decide separately whether development builds should pin a toolchain.
- [ ] Keep locked dependencies and both required `syn` major versions; inspect target-specific duplicate crates before attempting unification.
- [ ] Check winapi-target license coverage against actual shipped targets; generate/verify notice inventory from the final dependency graph.
- [ ] Add a maintained advisory check to CI, with a documented policy for reviewing results and local patches.

**Locations:** `Cargo.toml:1–20`, `vendor/vt100/Cargo.toml:57–75`, `Cargo.lock`, `THIRD_PARTY_LICENSES.txt`, `.github/workflows/ci.yml`.  
**Acceptance:** Unix no longer compiles unused Base64, Windows still builds, the declared Rust minimum passes, and notice/advisory checks have reviewable results.  
**Safety:** Base64 target scoping is safe; dependency upgrades, vendor pruning, and support-version changes are conditional.

### P14 — documentation aligned with verified behavior

- [ ] Update geometry, reset/clear, hyperlinks, signals, and unsupported-protocol limits alongside their fixes.
- [ ] Clarify source-only shell helpers; only add them to the npm package if that distribution path is desired.
- [ ] Explain which native wrapper options the npm launchers expose and how to invoke `rtl` directly for others.
- [ ] Document local/CI Windows artifact requirements and package verification steps.
- [ ] Replace broad compatibility claims with an explicit tested matrix, including unverified live-agent/accessibility behavior.
- [ ] Keep README focused on purpose, installation, keys, and limits. Move build/architecture/benchmark/packaging material to contributor documentation where useful, remove obsolete UI/catalog/tool-history prose, and retain meaningful compatibility limitations rather than deleting them as a development diary.
- [ ] State that shell helpers target source/Cargo installations unless another supported distribution is added. Use a generic interactive-terminal diagnostic instead of requiring VS Code by name.
- [ ] Add a project changelog from verified release changes and expose `terminal-rtl --version` from the npm manifest, with a small CLI behavior test.

**Locations:** `README.md`, `vendor/vt100/PATCHES.md`, `package.json`.  
**Acceptance:** examples match shipped files and tested behavior; no unsupported guarantees about selective history clearing, arbitrary protocols, or hard-kill cleanup.  
**Safety:** safe when documentation describes the final implementation.

## Phase 4 — optional improvements after correctness fixes

- [ ] **History/render performance:** borrow viewport rows rather than cloning all retained history; short-circuit speaker-label detection when disabled. Benchmark long-history browsing and hyperlink-heavy frames, not just 24×100 live screens. (`src/session.rs:359–363,533–559`, `src/display.rs:387–426`, `src/pretty.rs:147–169`.)
- [ ] **Idle CPU:** measure idle wakeups and input latency before changing the 4 ms poll interval or introducing another event architecture. (`src/session.rs:398`.)
- [ ] **Library coupling:** remove `clap::ValueEnum` from the library only if a real library-consumer benefit justifies the conversion code. (`src/display.rs:4,8–14`.)
- [ ] **Testability:** add unit tests in the binary module or extract pure helpers; moving the complete session engine into the public library is not required. (`src/main.rs:1`, `src/session.rs`.)
- [ ] **Session structure/constants:** consolidate margin/footer geometry before considering a `Session` struct; use named harness dimensions. Consider a typed agent enum only if it simplifies callers while preserving validation for library consumers. Do not remove defensive public-API validation solely because clap validates one caller.
- [ ] **Theme policy:** fixed dark OSC 10/11 replies are documented but can influence agent theme detection. Consider an explicit theme setting and tests if light-theme users are supported; do not forward host queries without a reliable reply transport.
- [ ] **CI efficiency and JS consistency:** measure build time before adding a Rust cache; consider main-only push plus PR checks and concurrency cancellation according to desired branch coverage. Preserve Node/Bun compatibility tests. Add a small JS formatting policy if it reduces recurring churn.
- [ ] **Feature controls:** an attribution opt-out or independent labels setting may improve user choice, but removing branding/labels is a product decision. Keep it separate from geometry fixes.
- [ ] **Platform/release expansion:** consider Linux bundling after defining libc compatibility and testing an extracted package. Additional architectures and tag-driven releases are separate scope. A future publishing workflow should use reviewed permissions/authentication/provenance; do not add secrets or publish as part of remediation.
- [ ] **Notice generation/upstream maintenance:** introduce deterministic notice generation and local-pack validation before changing tracked notices. Explore upstreaming independently reviewed parser patches, including hyperlink support; remove vendoring only when an upstream release preserves all required behavior and regressions. Opening upstream PRs requires separate authorization.
- [ ] **Small cleanup:** simplify demonstrably redundant prefix guards and explicit default paths only after tests; retain locally useful start/grok scripts unless maintainers intentionally drop that workflow. A shared platform manifest is optional if it reduces actual drift rather than adding indirection for two targets.
- [ ] **Root binary fallback:** confirm manual/external layouts before removing `packaged` from the launcher search order. (`bin/native-launchers.mjs:25–27`.)
- [ ] **Local artifacts:** archive/remove old tarballs only after deciding rollback needs; clean `target/` only when no local launcher relies on its release binary. Preserve `native/` for local packing and `node_modules/` when running JS tests without reinstalling. (`.gitignore:1–6`, `dist/`, `target/`, `native/`.)

**Acceptance:** each optimization has measured benefit or a clear maintenance benefit, preserves regression behavior, and does not introduce broad vendor churn. Cleanup remains separately authorized.

## Suggested implementation sequence

1. Unblock Windows verification first: P00A; then geometry regressions and fixes P01/P02, including the shared resize transition.
2. Escape parser boundary and complete-link handling: P03 plus parsing portions of P06.
3. Graceful shutdown: P04.
4. Windows resolution and shared fixtures: P05.
5. History delivery, replay, and clear/reset semantics: remaining P06, P07, P08.
6. Paste/prose investigation and optional interaction decisions: P09/P15/P10.
7. Package validation, platform coverage, and dependency hygiene: P11–P13.
8. Final documentation and release completeness: P14/P00.
9. Optional measured improvements and cleanup.

Keep changes reviewable by root cause. Add focused regressions before each behavioral fix; do not combine all parser, packaging, and performance work into one change.

## Release acceptance checklist

- [ ] All new focused regressions pass, including geometry cases in debug and release.
- [ ] Windows no longer stalls on cursor inheritance, and the package job produces an artifact from the proposed release commit.
- [ ] `cargo fmt --check` passes for project-owned code.
- [ ] `cargo clippy --all-targets --locked -- -D warnings` passes.
- [ ] `cargo test --locked` passes on macOS, Windows, and Linux.
- [ ] Release builds succeed for the intended native targets.
- [ ] `bun test` and `node --test tests/*.test.mjs` pass with installed locked development dependencies.
- [ ] Minimum-supported Node and Rust versions are tested.
- [ ] Native Windows shim/ConPTY behavior and Unix signal cleanup are verified.
- [ ] Independent host tests cover history preservation/clearing, complete links, replay, and resize boundaries.
- [ ] Package contents, formats, versions, permissions, notices, and extracted launch behavior pass validation.
- [ ] Advisory results are reviewed; local parser changes are not assumed covered by upstream version scans.
- [ ] A fresh checkout of the proposed commit passes without untracked source or local artifact fallbacks.
- [ ] Remaining limitations and deferred product decisions are explicit in the release notes.

## Highest-value outcomes

1. Active sessions survive resizing and minimal terminal dimensions.
2. Malformed escape output cannot grow parser memory without a bound.
3. Catchable termination restores the user's terminal.
4. Windows npm-installed agents resolve and launch through verified paths.
5. History and hyperlinks remain complete under bursts, resets, and replay, with a clear policy for pre-session host history.
