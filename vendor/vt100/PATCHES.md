# Local changes to vt100 0.16.2

Upstream: https://github.com/doy/vt100-rust (MIT; LICENSE retained).

- Retain rows scrolled off a region whose top is row zero, even when its bottom
  excludes a pinned composer. Codex uses this when replaying resumed history.
- Expose a total and incremental iterator for normal-screen history so Terminal
  RTL can mirror those rows into the host terminal's native scrollback.
- Expose saved-history clearing for CSI 3 J, preserving the visible composer.
- Store immutable hyperlink metadata alongside cells, including wide-cell
  continuations. Overwrite and erase remove old links; scrolling, cloning, and
  resizing preserve them. OSC 8 parsing and host serialization live in the parent
  project. Link targets are bounded and reject control characters.

- Preserve history at its original width; expose a clipped/padded viewport and
  original-width live rows for safe rendering and replay. Repair truncated wide
  cells, clamp empty dimensions to one, and handle wrapping in a single row.
- Replace a wide glyph with U+FFFD in a one-column grid.
- Deliver history synchronously before retention eviction, including with zero
  retained rows. Pending delivery is bounded to one terminal operation.
- Track DEC wraparound, report origin-relative cursor coordinates, expose RIS
  generations, and reset callback-owned modes on RIS.
- Use the locally patched `vte` parser (../vte/PATCHES.md) for bounded OSC and
  complete hyperlinks. The parent Protocol owns validated metadata.

Regression coverage lives in tests/regression/audit.rs, tests/display/rendering.rs, tests/terminal/protocol.rs,
and tests/pty/. Upstream dev-dependency declarations are retained to support
restoring upstream tests; they are not runtime dependencies.
