# Release validation and packaging

## Validation and release gates

| Environment | Current evidence |
| --- | --- |
| macOS arm64 | Local debug/release, PTY lifecycle/geometry, Node 18.0 and 24, Bun 1.4.2, xterm.js |
| Rust 1.88 | Local tests and Windows GNU cross-check of the locked graph |
| Windows x64 GNU | Local cross-build; native runtime remains unverified |
| Windows x64 MSVC | 0.1.6 native CI and extracted-package checks passed; new source requires revalidation |
| Linux x64 | 0.1.6 source CI passed; no bundled Linux binary |
| Actual Codex/Grok conversations and accessibility | Not certified by synthetic tests |

Pre-release local validation on 2026-09-07:

- `cargo test --all-targets --locked`: 64 passes from a clean source snapshot;
  `cargo +1.88.0 test --all-targets --locked`: 64 passes.
- `cargo test --release --locked --test audit`: 11 passes. Vendored vte: 31
  default-feature tests and 34 no-default-feature tests pass.
- `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
  macOS release build, Windows GNU release cross-build, and Rust 1.88 Windows GNU
  all-target check pass.
- `node scripts/checks/test-node.mjs` under Node 24 and Node 18.0, and `bun test`:
  nine passes, two Windows-only skips each.
- `bun pm pack --destination dist` in a temporary source copy passes prepack;
  `node scripts/packaging/smoke-package.mjs <archive>` and its Bun invocation pass on macOS
  with the actual extracted engine, including piped and interactive launches.
  This local validation archive contains a GNU Windows cross-build; it is not
  qualified for publication. Existing repository native binaries were preserved.

Release 0.1.6 passed all eight jobs in
[CI run 34110241764](https://github.com/Quegenx/terminal-rtl/actions/runs/34110241764)
at commit `76cf5a3`, including macOS and Windows extracted-package checks.
The published archive came from that run. This evidence applies to the release
commit; subsequent source changes require their own validation.

The updated workflow checks all three OSes, minimum runtime versions, notices,
and advisories before assembling `npm-package`. Separate macOS/Windows jobs
extract that archive and test its actual bundled engine, piped launcher, and
interactive launcher. A `target/` directory or installed `rtl` cannot substitute
for the bundled engine in those checks. All jobs, including package smoke jobs,
must succeed before release.

Reviewed and SHA-pinned action releases: [checkout 7.0.1](https://github.com/actions/checkout/releases/tag/v7.0.1),
[upload-artifact 7.0.1](https://github.com/actions/upload-artifact/releases/tag/v7.0.1),
[download-artifact 8.0.1](https://github.com/actions/download-artifact/releases/tag/v8.0.1),
and [setup-node 7.0.0](https://github.com/actions/setup-node/releases/tag/v7.0.0).
They use Node 24 / ESM; use current GitHub-hosted runners (minimum runner 2.327.1;
authenticated checkout in Docker needs 2.329.0). These runtime updates are
separate from the ConPTY cursor-handshake fix.

Windows input qualification covers normal bracketed paste and native key events.
Older ConPTY versions can discard unfinished VT sequences between pipe writes,
before the application receives them. The fragmented-paste test therefore uses
complete Win32 envelopes on Windows while splitting the enclosed paste delimiters
across console records. The decoder also checks Hebrew, emoji, nested openers,
and literal control characters without interpreting them as shortcuts.

## Packaging

The supported release path uses the native macOS arm64 and Windows x64 MSVC CI
artifacts. Local GNU Windows builds remain useful for testing but need native
qualification before shipping. The validator checks architecture/format, regular
files, Unix execute permissions, embedded version markers, and native `--version`
where executable on the current host. It also compares Cargo and npm versions.

```sh
# Place current, verified artifacts in native/darwin-arm64/rtl and
# native/win32-x64/rtl.exe first. Preserve older files needed for rollback.
node scripts/packaging/notices.mjs
node scripts/packaging/check-package.mjs
bun pm pack --destination dist
# On each bundled platform, using the generated archive's actual filename:
node scripts/packaging/smoke-package.mjs path/to/package.tgz
```

Packing rejects older binaries without the build marker, even if their file
sizes look plausible. A fresh source checkout must build and test without
untracked modules or preexisting native artifacts. A clean source snapshot is
useful locally; it is not a substitute for validating the eventual release commit.

Publishing remains a separate maintainer operation after authentication and
review: `bun publish path/to/verified-package.tgz`. Remediation does not publish,
push, or trigger remote workflows. Keep old archives, manual root-binary layouts,
and installed dependencies until their rollback or local-use requirements are
resolved. Linux bundling, upstream PRs, and tag-driven publishing remain separate
projects.
