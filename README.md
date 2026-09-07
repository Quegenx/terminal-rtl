# Terminal RTL

**Powered by: Gal Havkin**

Hebrew RTL display correction and lightweight formatting for terminal AI agents.
Run the original Codex or Grok CLI inside your existing terminal. Child text and
input stay in logical order; only the displayed rows are reordered. No AI API,
API key, or replacement agent UI is involved.

## Install and run

Install the original Codex or Grok CLI first, then:

```sh
npx terminal-rtl codex
npx terminal-rtl grok

# Bun, including machines without Node:
bunx --bun terminal-rtl codex
bunx --bun terminal-rtl grok
```

The RTL engine is included and selected automatically on macOS Apple Silicon
and Windows x64. Users do not install Rust, run build commands, download a
separate engine, or rebuild Codex/Grok. Other platforms are not bundled yet.
The original Codex/Grok CLI remains responsible for its own authentication and
configuration. The launchers support Node 18+ and Bun.

Agent arguments pass through unchanged to native executables. Piped and nested
(`RTL_ACTIVE`) invocations run the agent directly. Native status lines, slash
commands, themes, model selection, and key bindings belong to the original CLI.
CLI upgrades take effect on the next launch, but unfamiliar terminal protocols
or layouts may need compatibility changes.

On Windows, bare commands follow supported PATHEXT entries (`.COM`, `.EXE`,
`.BAT`, `.CMD`); extensionless npm shell shims and `.ps1` files are skipped.
Batch files use a quoted PowerShell bridge and remain subject to Windows
PowerShell's native argument rules, particularly for empty arguments and
metacharacters. Native executables use direct argument passing.

## Display and controls

Interactive npm launchers enable inline history, a creator footer, Markdown
formatting, and an eight-column `you:` / `codex:` / `grok:` margin. Message labels
are inferred from native markers and can miss or misclassify unfamiliar layouts.
The margin disappears below ten host columns; the footer disappears at one row.

Formatting recognizes complete Markdown tables, headings, lists, quotes, and
code fences. Existing CLI colors take precedence. Active input and incomplete
tables are left alone. Formatting and RTL correction are enabled automatically by the launcher.

| Keys | Action |
| --- | --- |
| Ctrl+] then `r` | Toggle Hebrew correction |
| Ctrl+] then `q` | Terminate the wrapped command and exit |
| Ctrl+] twice | Send a literal Ctrl+] |
| Shift+PageUp / Shift+PageDown | Browse retained normal-screen rows |
| Any normal typing key | Return from retained history to live input |
| Ctrl+C | Send Ctrl+C to the child |
| Shift+Enter | Preserve modified Enter for a draft newline |

Ctrl+] waits for the next key without a timeout or pending indicator. An
unrecognized second key forwards both the prefix and that key. Window/icon
titles (OSC 0/1/2) are deliberately not forwarded.

In inline mode, use the host's mouse wheel, scrollbar, and ordinary text
selection. If the child requests native mouse events, those events retain their
native meaning; hold Shift for host selection where supported. In non-inline
mode the wheel browses wrapper history unless the child handles mouse events.

Some terminals intercept keys or encode Enter and Shift+Enter identically. Bind
Shift+Enter to `\u001b[13;2u` and Ctrl+] to `\u001d` if needed. The wrapper cannot
distinguish identical input bytes.

## History, links, and resizing

Inline output reaches host scrollback before the wrapper's retention ring can
evict it. Inline delivery continues even when wrapper retention is disabled. Delivery is synchronous and bounded; a slow host
applies backpressure to the child.

Child `CSI 3 J` clears only wrapper-retained rows. It does not erase pre-session
shell history or rows already mirrored to the host. RIS resets the child screen
and negotiated modes without clearing the host's earlier output.

Saved rows retain their original width. Browsing pads or clips them to the
current viewport without reflow; widening again restores saved content. Live
rows are resized by clipping/padding. A clipped wide glyph is cleared, and a
wide glyph in a one-column terminal is replaced by U+FFFD (`�`).

The npm launchers use inline history automatically. The engine also supports
alternate-screen replay for development; those options are documented in the
[contributor guide](docs/development.md#advanced-engine-options-maintainers).

OSC 8 hyperlinks retain complete destinations through reordering, redraw,
wrapping, history, and exit replay. Targets are limited to 8,192 UTF-8 bytes;
invalid or oversized sequences are rejected completely. OSC accumulation is
bounded before dispatch, including fragmented and unterminated output. Use the
host's usual open-link gesture. Plain URLs still rely on host auto-detection.

## Limits

- Mouse-selected Hebrew is copied in visual order; the wrapper cannot change
  the terminal's selection API. Left/Right movement follows the child's logical
  editing model even though cursor and mouse coordinates are mapped for display.
- Reordering is per displayed field/row. Complex wrapped paragraphs, unusual
  layouts, emoji clusters, and accessibility tools require more validation.
  Arabic shaping is not implemented.
- Ordinary Hebrew, Unicode, and multiline paste text is preserved. Literal
  bracketed-paste closer bytes end the host's paste event; bytes after them are
  keys. The wire protocol cannot represent that closer as literal paste text.
  The public paste serializer does not sanitize arbitrary terminal controls.
- Images, clipboard OSCs, full kitty keyboard reporting, child-requested window
  manipulation, and window/icon titles are unsupported. OSC 10/11 query replies
  use a fixed dark palette rather than probing the host theme.
- Unix SIGTERM/SIGHUP/SIGINT requests terminate the child and restore terminal
  modes through normal cleanup; signal exits use `128 + signal`. Final output
  draining is bounded to 500 ms after observed child exit or a shutdown request.
  Windows console-control cleanup is implemented but still needs native runtime
  qualification. Windows forced process termination and Unix SIGKILL cannot
  guarantee restoration. Use `reset` or open a new terminal after a hard kill.

Development validation, platform release checks, and engine configuration are
covered in the [documentation index](docs/README.md). They are maintainer tasks,
not installation steps for users.
