# Terminal RTL

**Powered by: Gal Havkin**

The creator credit stays visible in a reserved footer row during interactive sessions.

Hebrew RTL display correction and lightweight formatting for native terminal AI agents.

Run the original Codex or Grok CLI with RTL correction and lightweight formatting.

## Install and run

```sh
npx terminal-rtl codex
npx terminal-rtl grok

# Bun, including systems without Node installed:
bunx --bun terminal-rtl codex
bunx --bun terminal-rtl grok
```

For permanent commands:

```sh
bun add --global terminal-rtl
# Or: npm install --global terminal-rtl
codex-rtl
grok-rtl
```

Install the original Codex or Grok CLI first. The package includes the native RTL
engine and selects it automatically; no ZIP downloads or Rust installation are
needed. Bundled platforms: macOS Apple Silicon and Windows x64. Other platforms
currently need a source build and `RTL_BIN` pointing to the resulting executable.
The JavaScript launchers work with Node 18+ or Bun and have no package dependencies.
Node-shebang commands installed globally require Node on PATH; Bun-only users can
use `bunx --bun` as shown above.

Native status lines, slash commands, themes, configuration, model selection, and
key bindings remain controlled by the original CLI. All CLI arguments pass through
unchanged. No extra agent tool is installed. `RTL_CODEX_BIN`, `RTL_GROK_BIN`, and
`RTL_BIN` select explicit executables. Piped/noninteractive commands run directly
so machine-readable output and exit codes stay native.

The native launchers enable conservative display formatting: aligned Markdown
tables get Unicode separators and bold headers. H1 is bold and underlined;
H2 and H3 use distinct accent colors. Existing CLI colors take precedence.
List markers are bold, quotes italic, and fenced code stays readable with dim
fence delimiters. Active input and incomplete tables are left alone.

A small eight-column margin shows `you:` and `codex:` / `grok:` beside recognized
native message starts. The child terminal is resized to fit this margin, and
cursor/mouse coordinates account for it. Labels are inferred from native markers
(including Grok's full-screen and minimal layouts), so unfamiliar layouts can miss labels or
misclassify matching text. No semantic conversation API is used.

Set `RTL_PRETTY=0` to use only RTL correction. For direct invocation, use
`rtl --pretty --agent-label codex -- codex` (or `grok` for both names).
Native Unicode tables keep their own presentation. Formatting and labels affect
the live screen; retained output replay uses original text with RTL correction.

This is automatic output formatting, **not the entire json-render interactive
component catalog**. Ordinary terminal output cannot define arbitrary form
state and actions. The obsolete replacement UI and its `rtl-ui` command have
been removed; use `codex-rtl` or `grok-rtl` instead.

CLI updates are picked up on the next launch because the actual installed
executables are used. No CLI source fork or pinned app-server protocol is
involved in native mode. New terminal escape protocols may still need wrapper
compatibility fixes; compatibility with every future release is not guaranteed.

Run agents with readable Hebrew **inside your existing VS Code terminal**.
`rtl` starts the command in a pseudo-terminal, interprets its screen updates,
and reorders Hebrew for display. Prompts and agent output remain in logical
order inside the child process. It does not call an AI API or require an API key.

This is an initial working implementation, not a guarantee of compatibility
with every agent or terminal protocol. macOS PTY integration tests pass locally;
Windows code has been compiled into a native executable. Native Windows runtime
verification remains necessary. CI includes the same PTY tests on Windows.

## Build from source

Install [Rust stable](https://doc.rust-lang.org/stable/book/ch01-01-installation.html).
On Windows, use the MSVC toolchain and install the C++ build tools requested by
the Rust installer. On macOS, install Xcode Command Line Tools if prompted.

In this project directory, on either platform:

```sh
cargo install --path . --locked
rtl --demo
rtl codex
```

The demo shows mixed Hebrew/English, numbers, colours, streaming, and Hebrew
input without calling an agent. `rtl --doctor` prints basic terminal diagnostics.

Alternatively, build without installing:

```sh
cargo build --release --locked
```

Run `./target/release/rtl --demo` on Mac, or
`.\target\release\rtl.exe --demo` in Windows PowerShell.

Agent arguments follow the command unchanged:

```sh
rtl codex --help
rtl gemini
rtl --direction ltr -- your-agent --some-agent-option
```

You can also wrap a shell once and run commands inside it:

```sh
rtl zsh
```

On Windows:

```powershell
rtl powershell -NoLogo
```

Native Windows uses ConPTY; WSL is not required. Windows `.cmd`/`.bat` launchers
(including common npm shims) are launched through an encoded PowerShell command.
Arguments to those batch files are subject to Windows PowerShell's native command
argument rules. If a complex argument behaves differently, start a wrapped shell
and run the command there. Native executable arguments do not go through a shell.

## Make selected agent commands automatic

Optional helpers let you continue typing `codex` instead of `rtl codex`.
They affect the current shell only unless you add the source commands to your
shell profile. They refuse to replace existing functions or aliases.

Mac / zsh, from this project directory:

```zsh
source shell/rtl.zsh
rtl-wrap codex gemini
```

Windows / PowerShell:

```powershell
. .\shell\rtl.ps1
Enable-RtlCommand -Name codex, gemini
```

Use the full path to these files when adding them to a shell profile. Remove the
profile lines and open a fresh terminal to undo the integration. Helpers avoid
double wrapping when already inside an `rtl` shell.

An agent that **already corrects Hebrew itself** should usually run without this
wrapper. Otherwise, toggle correction off or start with `rtl --no-bidi agent`.
There is no reliable way to automatically detect whether Hebrew was pre-reversed.

## During a session

| Keys | Action |
| --- | --- |
| Ctrl+] then `r` | Toggle Hebrew correction; the agent keeps running |
| Ctrl+] then `q` | Terminate the wrapped command and exit |
| Ctrl+] twice | Send a literal Ctrl+] to the child |
| Mouse wheel / trackpad | Scroll retained conversation history; apps with native mouse support handle their own scrolling |
| Shift+PageUp / Shift+PageDown | Browse retained normal-screen scrollback |
| Any normal typing key | Return from scrollback to live input |
| Ctrl+C | Send Ctrl+C to the child as usual |

Some VS Code keybindings intercept these keys. If so, assign a terminal
`sendSequence` binding for `\u001d` or the relevant key in VS Code.

The native launchers use `--inline`: finalized output is mirrored into the host
terminal's normal scrollback as it arrives, including resumed conversation
history. Use the terminal's mouse wheel, scrollbar, and ordinary drag-to-select
and copy commands. Codex does not require mouse capture in this mode. Agents
that request mouse events (such as Grok's full-screen UI) retain their native
mouse handling; hold Shift to select text through the host in those modes.

Direct `rtl` invocations without `--inline` still use an alternate screen with
wrapper-managed scrollback and optional output replay on exit (`--no-replay`
disables replay). Child alternate screens do not produce retained history.
The pinned credit remains on the live viewport; it scrolls out of view when
browsing older output through the host's history.

`--record PATH` optionally writes the child's **original raw PTY output**, including
ANSI sequences, to a new file. No recording is made by default, existing files
are never overwritten, and user input is recorded only if the child echoes it.
The file may include conversation contents. It is not a timing-aware recording.

## What is implemented

- Unix PTY and native Windows ConPTY launch, input, resize, exit status, cleanup.
- Stateful ANSI parsing through a patched `vt100` (which uses `vte`), including fragmented
  UTF-8, cursor movement, erase, scrolling, standard colours, and alternate screens.
- Unicode bidi reordering, bracket mirroring, combining-mark attachment, and
  wide-cell preservation. English-only text is not reordered.
- Fixed box-drawing borders, indentation, common prompt markers, and gaps of two
  or more spaces. These delimit independent text fields.
- Logical/visual cursor and mouse-coordinate mapping, standard keyboard shortcuts,
  bracketed paste, and standard xterm mouse events.
- Logical cursor-position replies, basic device attributes, size queries, focus
  events, and synchronized-output handling with a timeout.
- Changed-row rendering, a 60 FPS cap, and a bounded output queue.

## Current limits

- **Mouse-selected Hebrew copies in visual order.** A terminal wrapper cannot
  make VS Code's selection API recover the original logical text. English-only
  commands remain in normal order. Recording preserves the original output.
- This changes text display, not the child application's editing model. Cursor
  positions are mapped, but Left/Right still follow the child's logical movement.
- Reordering is per displayed text field/row. Complex soft-wrapped paragraphs,
  unusual table layouts, and programs that position every glyph themselves need
  additional compatibility testing. The border/column heuristic can be overridden
  only by disabling correction; it is not a code/Markdown parser.
- Advanced emoji clusters inherit the parser's cell-width limitations. Arabic
  shaping is not provided; Hebrew is the primary target.
- OSC hyperlinks display their text but do not retain link metadata. Images,
  clipboard OSCs, extended kitty keyboard protocols, and application-requested
  window manipulation are not supported. Colour queries use a fixed dark palette.
- Forced OS termination cannot always restore terminal state. After a hard kill,
  `reset` on Unix or opening a fresh terminal restores a usable terminal.
- Agents that depend on unsupported escape sequences may need changes. Codex's
  interactive startup and Hebrew prompt input were smoke-tested on this Mac,
  without submitting a prompt. Full conversational workflows still need testing.

## Development

```sh
cargo test --locked
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
bun install --frozen-lockfile
bun test
```

For a CPU-only display benchmark, run `cargo run --release --example benchmark`.
On the development Apple Silicon Mac, 2,000 full 24x100 screen updates averaged
0.300 ms per frame. This excludes terminal I/O and does not measure an agent's
end-to-end latency.

The PTY tests launch the actual wrapper, stream split UTF-8, verify logical
cursor replies, paste Hebrew, resize the terminal, toggle correction, forward
Ctrl+C, check exit codes, and check terminal restoration. Other tests cover
mixed text, punctuation, marks, colours, borders, wide glyphs, wrapping, erase,
alternate screens, scrollback, and input encoding.

The host scrollback test also feeds a synthetic PTY transcript into xterm.js,
independently of the wrapper's parser. It checks that resumed rows reach native
history and that the composer and footer remain visible. History is appended
with line feeds at the bottom of the viewport: `CSI S` can discard rows instead
of saving them in a host terminal.

`.github/workflows/ci.yml` tests and builds on macOS, Windows, and Linux. It uploads
native binaries as workflow artifacts; no remote workflow has been run merely
by creating these files locally.

## Architecture

```text
keyboard / paste ────────────────► child PTY (original text)
                                      │
                                      ▼
                               vt100 / vte parser
                                      │
                               logical screen cells
                                      │
                            Unicode bidi + mirroring
                                      │
                              changed-row renderer
                                      │
                                      ▼
                              existing VS Code terminal
```

Library APIs were checked using the Context7 and Mintlify MCPs and downloaded
crate source. Primary references:
[portable-pty](https://docs.rs/portable-pty/0.9.0/portable_pty/),
[vt100](https://docs.rs/vt100/0.16.2/vt100/),
[unicode-bidi](https://docs.rs/unicode-bidi/0.3.18/unicode_bidi/),
[crossterm](https://docs.rs/crossterm/0.29.0/crossterm/).

## Maintainer packaging

GitHub Actions builds and tests the native engine, then assembles a single npm
package containing macOS arm64 and Windows x64 binaries. Download its
`npm-package` artifact to get the publishable `.tgz`. Linux is tested at source
level but is not yet a bundled platform.

For local packing, place release binaries at `native/darwin-arm64/rtl` and
`native/win32-x64/rtl.exe`, keep `THIRD_PARTY_LICENSES.txt` current, then run
`bun pm pack --destination dist`. Packing fails if a required binary is missing.
There are no install scripts and no downloads at launch time.

Authenticate to npm, then publish the verified tarball with `bun publish ./path/to/terminal-rtl-0.1.4.tgz`.
Registry publication is separate from preparing the package. Homebrew is not
configured in this release.

The small local `vendor/vt100` patch preserves top-anchored scrolling regions
used by Codex resume and exposes incremental history for the native scrollback
mirror. Its upstream license and patch notes are included in that directory.
