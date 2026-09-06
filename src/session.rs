use std::{
    ffi::OsString,
    fs::File,
    io::{self, Read, Write},
    sync::mpsc::{self, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use crossterm::{
    cursor::Show,
    event::{
        self, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseEventKind,
    },
    execute,
    style::ResetColor,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};
use terminal_rtl::{
    display::{Renderer, visual_row},
    input::{key_bytes, mouse_bytes, paste_bytes},
    protocol::Protocol,
};

use crate::Args;

struct TerminalGuard {
    inline: bool,
}

impl TerminalGuard {
    fn enter(inline: bool) -> Result<Self> {
        terminal::enable_raw_mode()?;
        let guard = Self { inline };
        let mut out = io::stdout();
        if inline {
            // Start a fresh viewport without erasing the shell's earlier history.
            execute!(out, DisableMouseCapture)?;
            for _ in 0..terminal::size()?.1 {
                out.write_all(b"\r\n")?;
            }
        } else {
            execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
        }
        execute!(out, EnableBracketedPaste, EnableFocusChange)?;
        // Disable host wrapping: our logical screen already implements wrapping.
        io::stdout().write_all(b"\x1b[?7l\x1b[0m\x1b[2J\x1b[H")?;
        io::stdout().flush()?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = out.write_all(b"\x1b[?7h");
        let _ = execute!(
            out,
            DisableMouseCapture,
            DisableBracketedPaste,
            DisableFocusChange,
            ResetColor,
            Show
        );
        if self.inline {
            if let Ok((_, rows)) = terminal::size() {
                let _ = write!(out, "\x1b[{};1H\r\n", rows);
            }
        } else {
            let _ = execute!(out, LeaveAlternateScreen);
        }
        let _ = out.flush();
        let _ = terminal::disable_raw_mode();
    }
}

struct ChildGuard {
    child: Box<dyn Child + Send + Sync>,
    exited: bool,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.exited {
            let _ = self.child.kill();
        }
    }
}

enum Output {
    Data(Vec<u8>),
    End,
    Error(io::Error),
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.max(1),
        cols: cols.max(1),
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn command(args: &[OsString]) -> Result<CommandBuilder> {
    let mut cmd = platform_command(args);
    // portable-pty defaults to the user's home, not the caller's directory.
    cmd.cwd(std::env::current_dir().context("cannot resolve working directory")?);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("RTL_ACTIVE", "1");
    Ok(cmd)
}

#[cfg(not(windows))]
fn platform_command(args: &[OsString]) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(&args[0]);
    cmd.args(&args[1..]);
    cmd
}

#[cfg(windows)]
fn platform_command(args: &[OsString]) -> CommandBuilder {
    use base64::Engine as _;
    // ConPTY/CreateProcess cannot execute npm .cmd shims directly. Resolve the
    // executable ourselves and use a quoted, encoded PowerShell script only for
    // batch files. Native executables keep direct argument passing.
    let path = resolve_windows_command(&args[0]);
    let batch = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
    if !batch {
        let mut cmd = CommandBuilder::new(path);
        cmd.args(&args[1..]);
        return cmd;
    }
    let quoted = |s: &std::ffi::OsStr| format!("'{}'", s.to_string_lossy().replace('\'', "''"));
    let mut script = format!("& {}", quoted(path.as_os_str()));
    for arg in &args[1..] {
        script.push(' ');
        script.push_str(&quoted(arg));
    }
    script.push_str("; if ($null -ne $LASTEXITCODE) { exit $LASTEXITCODE } else { exit 1 }");
    let bytes: Vec<_> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut cmd = CommandBuilder::new("powershell.exe");
    cmd.args(["-NoLogo", "-NoProfile", "-EncodedCommand"]);
    cmd.arg(base64::engine::general_purpose::STANDARD.encode(bytes));
    cmd
}

#[cfg(windows)]
fn resolve_windows_command(name: &std::ffi::OsStr) -> std::path::PathBuf {
    use std::path::PathBuf;
    let candidate = PathBuf::from(name);
    let mut dirs = vec![PathBuf::new()];
    if candidate.components().count() == 1
        && let Some(path) = std::env::var_os("PATH")
    {
        dirs.extend(std::env::split_paths(&path));
    }
    let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    for dir in dirs {
        let path = dir.join(&candidate);
        if path.is_file() {
            return path;
        }
        if path.extension().is_none() {
            for ext in extensions.split(';').filter(|e| !e.is_empty()) {
                let mut full = path.as_os_str().to_os_string();
                full.push(ext);
                let full = PathBuf::from(full);
                if full.is_file() {
                    return full;
                }
            }
        }
    }
    candidate
}

pub fn run(args: &Args, mut recording: Option<File>) -> Result<u32> {
    let (cols, rows) = terminal::size().context("cannot read terminal size")?;
    let margin = if args.agent_label.is_some() { 8 } else { 0 };
    let child_size = |cols: u16, rows: u16| {
        size(
            cols.saturating_sub(margin),
            rows.saturating_sub(u16::from(args.attribution && rows > 1)),
        )
    };
    let initial = child_size(cols, rows);
    let pair = native_pty_system()
        .openpty(initial)
        .context("cannot create pseudo-terminal")?;
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;
    let child = pair
        .slave
        .spawn_command(command(&args.command)?)
        .with_context(|| format!("cannot launch {}", args.command[0].to_string_lossy()))?;
    let mut child = ChildGuard {
        child,
        exited: false,
    };
    drop(pair.slave);

    // Bound queued bytes (~512 KiB), so a noisy child cannot grow memory without limit.
    let (send, receive) = mpsc::sync_channel(64);
    thread::spawn(move || {
        let mut buffer = [0; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    let _ = send.send(Output::End);
                    break;
                }
                Ok(count) => {
                    if send.send(Output::Data(buffer[..count].to_vec())).is_err() {
                        break;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                // Unix PTYs commonly report EIO when the last slave closes.
                Err(e) if cfg!(unix) && e.raw_os_error() == Some(5) => {
                    let _ = send.send(Output::End);
                    break;
                }
                Err(e) => {
                    let _ = send.send(Output::Error(e));
                    break;
                }
            }
        }
    });

    let guard = TerminalGuard::enter(args.inline)?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<(u32, Vec<u8>)> {
            let mut parser = vt100::Parser::new_with_callbacks(
                initial.rows,
                initial.cols,
                usize::from(args.scrollback),
                Protocol::default(),
            );
            let mut renderer = Renderer::default();
            renderer.set_pretty(args.pretty);
            renderer.set_agent_label(args.agent_label.as_deref());
            renderer.set_attribution(args.attribution && rows > 1);
            let mut enabled = !args.no_bidi;
            let mut prefix = false;
            let mut scrollback = 0usize;
            let mut mirrored_history = 0u64;
            let mut mouse_capture = !args.inline;
            let mut dirty = true;
            let mut eof = false;
            let mut exit = None;
            let mut exit_time = None;
            let mut last_render = Instant::now() - Duration::from_secs(1);
            let mut sync_started = None;
            let mut current_size = (cols, rows);
            let mut last_size_check = Instant::now();
            let mut out = io::stdout();
            loop {
                // A resize signal/event can be coalesced or missed by a host. The
                // occasional size query also covers nested PTYs and ConPTY hosts.
                if last_size_check.elapsed() >= Duration::from_millis(200) {
                    last_size_check = Instant::now();
                    if let Ok((cols, rows)) = terminal::size() {
                        let next = child_size(cols, rows);
                        if (cols, rows) != current_size {
                            current_size = (cols, rows);
                            renderer.set_attribution(args.attribution && rows > 1);
                            parser.screen_mut().set_size(next.rows, next.cols);
                            pair.master.resize(next)?;
                            renderer.invalidate();
                            dirty = true;
                        }
                    }
                }
                // Time-slice output so continuous streaming cannot starve input.
                for _ in 0..64 {
                    match receive.try_recv() {
                        Ok(Output::Data(bytes)) => {
                            if let Some(file) = &mut recording {
                                file.write_all(&bytes).context("recording failed")?;
                            }
                            parser.process(&bytes);
                            dirty = true;
                            if exit.is_some() {
                                exit_time = Some(Instant::now());
                            }
                            if !parser.callbacks().replies.is_empty() {
                                let replies = std::mem::take(&mut parser.callbacks_mut().replies);
                                writer.write_all(&replies)?;
                                writer.flush()?;
                            }
                        }
                        Ok(Output::End) | Err(TryRecvError::Disconnected) => {
                            eof = true;
                            break;
                        }
                        Ok(Output::Error(e)) => return Err(e).context("reading agent output"),
                        Err(TryRecvError::Empty) => break,
                    }
                }
                if parser.callbacks().bell {
                    out.write_all(b"\x07")?;
                    out.flush()?;
                    parser.callbacks_mut().bell = false;
                }
                if parser.callbacks().synchronized_output {
                    if sync_started.is_none() {
                        sync_started = Some(Instant::now());
                    }
                } else {
                    sync_started = None;
                }
                let sync_ready =
                    sync_started.is_none_or(|t| t.elapsed() >= Duration::from_millis(250));
                if dirty
                    && (exit.is_some()
                        || (sync_ready && last_render.elapsed() >= Duration::from_millis(16)))
                {
                    if std::mem::take(&mut parser.callbacks_mut().clear_scrollback) && args.inline {
                        out.write_all(b"\x1b[3J")?;
                    }
                    if args.inline {
                        let total = parser.screen().scrollback_total();
                        if total < mirrored_history {
                            mirrored_history = 0;
                        }
                        let history: Vec<_> =
                            parser.screen().history_since(mirrored_history).collect();
                        renderer.append_history(
                            &history,
                            parser.screen().size().1,
                            enabled,
                            args.direction,
                            &mut out,
                        )?;
                        mirrored_history = total;
                    }
                    // Never leave the live parser in scrollback while processing bytes.
                    if scrollback > 0 && !parser.screen().alternate_screen() {
                        let mut view = parser.screen().clone();
                        view.set_scrollback(scrollback);
                        scrollback = view.scrollback();
                        renderer.render(&view, enabled, args.direction, &mut out)?;
                    } else {
                        scrollback = 0;
                        renderer.render(parser.screen(), enabled, args.direction, &mut out)?;
                    }
                    last_render = Instant::now();
                    dirty = false;
                }
                let wants_mouse = !args.inline
                    || (scrollback == 0
                        && parser.screen().mouse_protocol_mode() != vt100::MouseProtocolMode::None);
                if wants_mouse != mouse_capture {
                    if wants_mouse {
                        execute!(out, EnableMouseCapture)?;
                    } else {
                        execute!(out, DisableMouseCapture)?;
                    }
                    mouse_capture = wants_mouse;
                }
                if exit.is_none()
                    && let Some(status) = child.child.try_wait()?
                {
                    child.exited = true;
                    exit = Some(status.exit_code());
                    exit_time = Some(Instant::now());
                    dirty = true;
                }
                // Drain final PTY output. ConPTY may keep a pipe open after child exit.
                if let Some(code) = exit
                    && (eof || exit_time.is_some_and(|t| t.elapsed() >= Duration::from_millis(500)))
                {
                    let snapshot = final_screen(parser.screen(), enabled, args.direction);
                    return Ok((code, snapshot));
                }

                if !event::poll(Duration::from_millis(4))? {
                    continue;
                }
                let event = event::read()?;
                let bytes = match event {
                    Event::Key(key) => {
                        if key.kind == KeyEventKind::Release {
                            continue;
                        }
                        let is_prefix = key_bytes(key, false) == [0x1d];
                        if prefix {
                            prefix = false;
                            match key.code {
                                KeyCode::Char('r') if !is_prefix => {
                                    enabled = !enabled;
                                    dirty = true;
                                    continue;
                                }
                                KeyCode::Char('q') if !is_prefix => {
                                    child.child.kill()?;
                                    return Ok((
                                        130,
                                        final_screen(parser.screen(), enabled, args.direction),
                                    ));
                                }
                                _ => {
                                    if !is_prefix {
                                        writer.write_all(&[0x1d])?;
                                    }
                                    key_bytes(key, parser.screen().application_cursor())
                                }
                            }
                        } else if is_prefix {
                            prefix = true;
                            continue;
                        } else if key.modifiers.contains(KeyModifiers::SHIFT)
                            && matches!(key.code, KeyCode::PageUp | KeyCode::PageDown)
                        {
                            let step =
                                usize::from(parser.screen().size().0.saturating_sub(1).max(1));
                            scrollback = if key.code == KeyCode::PageUp {
                                scrollback
                                    .saturating_add(step)
                                    .min(usize::from(args.scrollback))
                            } else {
                                scrollback.saturating_sub(step)
                            };
                            dirty = true;
                            continue;
                        } else {
                            if scrollback > 0 {
                                scrollback = 0;
                                dirty = true;
                            }
                            key_bytes(key, parser.screen().application_cursor())
                        }
                    }
                    Event::Paste(text) => {
                        scrollback = 0;
                        dirty = true;
                        paste_bytes(&text, parser.screen().bracketed_paste())
                    }
                    Event::Resize(cols, rows) => {
                        let size = child_size(cols, rows);
                        current_size = (cols, rows);
                        renderer.set_attribution(args.attribution && rows > 1);
                        parser.screen_mut().set_size(size.rows, size.cols);
                        pair.master.resize(size)?;
                        renderer.invalidate();
                        dirty = true;
                        continue;
                    }
                    Event::FocusGained if parser.callbacks().focus_events => b"\x1b[I".to_vec(),
                    Event::FocusLost if parser.callbacks().focus_events => b"\x1b[O".to_vec(),
                    Event::Mouse(mouse)
                        if matches!(
                            mouse.kind,
                            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                        ) && (scrollback > 0
                            || parser.screen().mouse_protocol_mode()
                                == vt100::MouseProtocolMode::None) =>
                    {
                        // Keep wheel reports out of the prompt's Up/Down history handler.
                        // Apps with native mouse scrolling retain control while live.
                        if !parser.screen().alternate_screen() {
                            scrollback = if mouse.kind == MouseEventKind::ScrollUp {
                                scrollback
                                    .saturating_add(3)
                                    .min(usize::from(args.scrollback))
                            } else {
                                scrollback.saturating_sub(3)
                            };
                            dirty = true;
                        }
                        continue;
                    }
                    Event::Mouse(mouse)
                        if scrollback == 0
                            && mouse.column >= margin
                            && mouse.row < parser.screen().size().0 =>
                    {
                        mouse_bytes(
                            mouse,
                            parser.screen().mouse_protocol_mode(),
                            parser.screen().mouse_protocol_encoding(),
                            renderer.logical_column(mouse.row, mouse.column),
                        )
                    }
                    _ => Vec::new(),
                };
                if !bytes.is_empty() && exit.is_none() {
                    writer.write_all(&bytes).context("sending input to agent")?;
                    writer.flush()?;
                }
            }
        },
    ));
    drop(guard);
    // Restore the terminal even on a panic, then resume normal panic handling.
    let (code, snapshot) = match result {
        Ok(result) => result?,
        Err(panic) => std::panic::resume_unwind(panic),
    };
    if !args.no_replay && !args.inline {
        io::stdout().write_all(&snapshot)?;
        io::stdout().flush()?;
    }
    Ok(code)
}

fn final_screen(
    screen: &vt100::Screen,
    enabled: bool,
    direction: terminal_rtl::display::Direction,
) -> Vec<u8> {
    let (rows, cols) = screen.size();
    let mut view = screen.clone();
    view.set_scrollback(usize::MAX);
    let history = view.scrollback();
    let total = history + usize::from(rows);
    let mut lines = Vec::new();
    let mut position = 0;
    while position < total {
        let start = position.min(history);
        view.set_scrollback(history - start);
        for row in (position - start) as u16..rows {
            let cells: Vec<_> = (0..cols)
                .map(|col| view.cell(row, col).unwrap().clone())
                .collect();
            let visual = visual_row(&cells, enabled, direction);
            let text: String = visual.glyphs.iter().map(|g| g.text.as_str()).collect();
            lines.push(text.trim_end().to_owned());
            position += 1;
        }
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        Vec::new()
    } else {
        format!("{}\n", lines.join("\n")).into_bytes()
    }
}
