# Changelog

## 0.1.6 — 2026-09-07

- Group runtime, display, terminal, test, and packaging code by responsibility;
  move contributor documentation into `docs/` and enforce a 300-line first-party
  file limit in CI. Public library paths and npm executable names remain stable.
- Fix historical-row rendering/replay after resize, truncated wide cells,
  one-column glyphs, and single-row wrapping. Hide decorations when space is scarce.
- Bound OSC accumulation before dispatch and reject malformed/truncated targets;
  preserve complete semicolon-bearing hyperlinks through live output and replay.
- Deliver inline history independently of retention, including `--scrollback 0`;
  preserve pre-session host history across child clears and resets.
- Preserve input arriving during Unix resize with Crossterm's TTY polling backend.
- Handle DEC wraparound, origin-relative cursor reports, and callback state resets.
- Restore terminal modes after catchable Unix termination; add Windows console
  cleanup and closing-pipe handling pending native qualification.
- Skip extensionless npm shell shims on Windows and align PATHEXT fallbacks.
- Add `--layout prose`, npm `--version`, and accurate timestamp-label boundaries.
- Read history viewports without cloning the full retained screen.
- Add geometry/lifecycle/paste/recording/protocol, independent host, launcher,
  shell-helper, package-negative, and minimum-runtime checks.
- Declare Rust 1.88, scope Base64 to Windows, generate/verify notices, and add
  advisory and extracted-package gates to CI.

Native Windows/updated remote CI and live-agent qualification remain release gates.
