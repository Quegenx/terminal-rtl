# Changelog

## 0.1.8 — 2026-09-11

- Coalesce host-terminal writes at frame boundaries during inline streaming.
- Preserve shifted render-cache rows after delivering inline history instead of
  repainting the full viewport.
- Avoid repainting an unchanged attribution footer on every frame.

## 0.1.7 — 2026-09-07

- Simplify child-output EOF handling and consolidate repeated test setup.
- Keep the public README focused on installation, usage, and known limitations.

## 0.1.6 — 2026-09-07

- Preserve native Windows key modifiers and bracketed paste across console reads.
- Restore the original Windows console input mode after exit.
- Keep the Windows child at least two columns wide and pause drawing in a
  one-column host to avoid ConPTY stalls with wide characters.

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
- Fix Windows inline startup when mouse capture has never been enabled;
  validate ConPTY screen updates and VT-input fixtures using native CI.
- Add `--layout prose`, npm `--version`, and accurate timestamp-label boundaries.
- Read history viewports without cloning the full retained screen.
- Add geometry/lifecycle/paste/recording/protocol, independent host, launcher,
  shell-helper, package-negative, and minimum-runtime checks.
- Declare Rust 1.88, scope Base64 to Windows, generate/verify notices, and add
  advisory and extracted-package gates to CI.

Release validation and remaining limitations are documented in [Releases](releases.md).
