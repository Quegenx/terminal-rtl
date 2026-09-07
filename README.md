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

Agent arguments pass through to the original CLI. Its status lines, slash
commands, themes, model selection, and key bindings remain available.

## Display and controls

Interactive npm launchers enable inline history, a creator footer, Markdown
formatting, and an eight-column `you:` / `codex:` / `grok:` margin. Message labels
are inferred from native markers and can miss or misclassify unfamiliar layouts.
The margin disappears below ten host columns; the footer disappears at one row.

Formatting recognizes complete Markdown tables, headings, lists, quotes, and
code fences. Existing CLI colors take precedence. Active input and incomplete
tables are left alone. Formatting and RTL correction are enabled automatically.

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
unrecognized second key forwards both the prefix and that key.

In inline mode, use the host's mouse wheel, scrollbar, and ordinary text
selection. If the agent uses mouse events, hold Shift for host selection where
your terminal supports it.

Some terminals intercept keys or encode Enter and Shift+Enter identically. Bind
Shift+Enter to `\u001b[13;2u` and Ctrl+] to `\u001d` if needed. The wrapper cannot
distinguish identical input bytes.

## History, links, and resizing

Output appears in your terminal's normal scrollback. Screen resets from the
agent preserve earlier shell history and output already delivered to the host.

Saved rows retain their original width. Browsing pads or clips them to the
current viewport without reflow; widening again restores saved content.

Terminal hyperlinks retain their destinations through reordering, resizing,
and history. Use your terminal's usual open-link gesture. Plain URLs rely on
the terminal's auto-detection.

## Limits

- Mouse-selected Hebrew is copied in visual order. Left/Right movement follows
  the agent's logical editing order.
- Reordering is per displayed field/row. Complex wrapped paragraphs, unusual
  layouts, emoji clusters, and accessibility tools require more validation.
  Arabic shaping is not implemented.
- Images, clipboard integration, window titles, and some advanced keyboard
  protocols are unsupported. Agent upgrades may introduce incompatible layouts.
- Windows batch launchers follow PowerShell's argument rules, including its
  limitations for empty arguments and metacharacters.
- Forced termination may leave terminal settings changed. Use `reset` or open
  a new terminal after a hard kill.

For source builds and advanced configuration, see the
[contributor documentation](docs/README.md).
