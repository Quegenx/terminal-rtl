# Local changes to vt100 0.16.2

Upstream: https://github.com/doy/vt100-rust (MIT; LICENSE retained).

- Retain rows scrolled off a region whose top is row zero, even when its bottom
  excludes a pinned composer. Codex uses this when replaying resumed history.
- Expose a total and incremental iterator for normal-screen history so Terminal
  RTL can mirror those rows into the host terminal's native scrollback.
- Expose saved-history clearing for CSI 3 J, preserving the visible composer.

Other parser behavior and dependencies remain upstream's. Regression coverage
lives in the parent project's tests/display.rs and tests/pty.rs.
