# Development

## Build from source

Install Rust 1.88 or newer. Use stable for normal development; there is no
project-wide toolchain pin. CI separately checks Rust 1.88. On macOS install
Xcode Command Line Tools; on Windows use the MSVC toolchain with C++ build tools.

```sh
cargo install --path . --locked
rtl --demo
cargo build --release --locked
```

The executable is `target/release/rtl` on Unix or `target\release\rtl.exe` on
Windows. If rustup is installed but Cargo is absent from PATH, add `~/.cargo/bin`
to PATH. `rtl --doctor` reports platform, terminal, size, and build identity.

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo test --release --locked --test audit
cargo test --manifest-path vendor/vte/Cargo.toml --locked --lib
cargo test --manifest-path vendor/vte/Cargo.toml --locked --lib --no-default-features
bun install --frozen-lockfile
bun test
node scripts/checks/test-node.mjs
node scripts/packaging/notices.mjs
node scripts/checks/structure.mjs
```

Clippy's default categories are enforced; pedantic/nursery/restriction groups
are not enabled wholesale. Both Node and Bun are supported and tested. The Node
runner also supports 18.0, which has `node:test` but lacks the later `--test`
command-line switch. Source helper tests use zsh on Unix and PowerShell on Windows.

PTY tests use synthetic children, never live agent prompts or API calls. Their
outer host parser answers fragmented cursor-inheritance queries exactly once
and tracks subsequent cursor position. Both `until` and `finish` use that same
receive path. A four-harness limit avoids exhausting small host PTY pools.
Every child scenario has a timeout and cleanup guard. Unix input uses Crossterm's
existing `use-dev-tty` backend: level-triggered polling avoids the reproduced
resize/input readiness stall in its default Mio path.

xterm.js independently checks history, clear/reset behavior, replay links, and
wide-row clipping. It reports absolute CPR in origin mode, unlike the
origin-relative policy specified by the [DEC VT100 manual](https://vt100.net/docs/vt100-ug/chapter3.html).
That difference is explicit in the comparison fixture; reset CPR agrees.

Windows resolver and argument-encoding fixtures are shared with JavaScript.
A cross-compile is not evidence that native ConPTY, npm shims, console-control
handlers, or all PowerShell batch argument cases work at runtime.

## Shell helpers

These files are for source/Cargo installations and are not npm payload files.
They refuse to replace existing functions or aliases and bypass nested wrapping.

```zsh
source shell/rtl.zsh
rtl-wrap codex gemini
```

```powershell
. .\shell\rtl.ps1
Enable-RtlCommand -Name codex, gemini
```

Use absolute paths if adding them to a shell profile. Remove the source command
and open a fresh terminal to undo that integration. The helpers do not edit a
profile automatically.

## Advanced engine options (maintainers)

`RTL_CODEX_BIN`, `RTL_GROK_BIN`, and `RTL_BIN` select explicit executables.
The npm launchers expose formatting through `RTL_PRETTY`; other wrapper options
are available through the source/Cargo-installed `rtl` command:

```sh
rtl --demo
rtl --doctor
rtl --layout prose -- codex
rtl --direction rtl -- grok
rtl --inline --attribution --pretty --agent-label codex -- codex
rtl --record raw-output.cast -- codex
rtl --no-bidi --no-replay -- zsh
```

The default `--layout columns` preserves indentation, prompt markers, box
borders, and gaps of two or more spaces as field boundaries. `--layout prose`
keeps repeated spaces inside one bidi field while preserving indentation,
prompt markers, and box borders. Use it when ordinary Hebrew sentences contain
repeated spaces. Whitespace cannot reliably identify semantic TUI columns.

`--record` creates a new file of original raw child output, including ANSI and
any input echoed by the child. Existing files are never overwritten; Unix files
are owner-only. Recordings can contain conversation contents.


Module ownership and the 300-line policy are in [Architecture](architecture.md).
Package and platform gates are in [Releases](releases.md).
