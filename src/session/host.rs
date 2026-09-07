use std::io::{self, Write};

use anyhow::Result;
use crossterm::{
    cursor::Show,
    event::{DisableBracketedPaste, DisableFocusChange, EnableBracketedPaste, EnableFocusChange},
    execute,
    style::ResetColor,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use portable_pty::PtySize;
use terminal_rtl::{display::Renderer, protocol::Protocol};

use crate::Args;

pub(super) struct TerminalGuard {
    inline: bool,
    keyboard_enhanced: bool,
}

impl TerminalGuard {
    pub(super) fn enter(inline: bool) -> Result<Self> {
        terminal::enable_raw_mode()?;
        let mut guard = Self {
            inline,
            keyboard_enhanced: false,
        };
        let mut out = io::stdout();
        if inline {
            // Start a fresh viewport without erasing the shell's earlier history.
            set_mouse_capture(&mut out, false)?;
            for _ in 0..terminal::size()?.1 {
                out.write_all(b"\r\n")?;
            }
        } else {
            execute!(out, EnterAlternateScreen)?;
            set_mouse_capture(&mut out, true)?;
        }
        execute!(out, EnableBracketedPaste, EnableFocusChange)?;
        // Ask capable hosts to distinguish Shift+Enter from Enter. Sending ANSI
        // directly also works on ConPTY; crossterm's command rejects Windows.
        out.write_all(b"\x1b[>1u")?;
        guard.keyboard_enhanced = true;
        // Disable host wrapping: our logical screen already implements wrapping.
        io::stdout().write_all(b"\x1b]8;;\x1b\\\x1b[?7l\x1b[0m\x1b[2J\x1b[H")?;
        io::stdout().flush()?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        if self.keyboard_enhanced {
            let _ = out.write_all(b"\x1b[<1u");
        }
        let _ = out.write_all(b"\x1b]8;;\x1b\\\x1b[?7h");
        let _ = set_mouse_capture(&mut out, false);
        let _ = execute!(
            out,
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

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows: rows.max(1),
        cols: cols.max(1),
        pixel_width: 0,
        pixel_height: 0,
    }
}

pub(super) fn child_geometry(args: &Args, cols: u16, rows: u16) -> (PtySize, bool) {
    // Decorations disappear before they consume the last child cell.
    let labels = args.agent_label.is_some() && cols >= 10;
    (
        size(
            cols.saturating_sub(if labels { 8 } else { 0 }),
            rows.saturating_sub(u16::from(args.attribution && rows > 1)),
        ),
        labels,
    )
}

pub(super) fn configure_renderer(renderer: &mut Renderer, args: &Args, cols: u16, rows: u16) {
    renderer.set_agent_label(if child_geometry(args, cols, rows).1 {
        args.agent_label.as_deref()
    } else {
        None
    });
    renderer.set_attribution(args.attribution && rows > 1);
}

pub(super) fn resize(
    master: &(dyn portable_pty::MasterPty + Send),
    parser: &mut vt100::Parser<Protocol>,
    renderer: &mut Renderer,
    args: &Args,
    dimensions: (u16, u16),
) -> Result<()> {
    let (cols, rows) = dimensions;
    let next = child_geometry(args, cols, rows).0;
    master.resize(next)?;
    parser.screen_mut().set_size(next.rows, next.cols);
    configure_renderer(renderer, args, cols, rows);
    renderer.invalidate();
    Ok(())
}

// Crossterm's Windows DisableMouseCapture requires an earlier enable call and
// otherwise fails with "Initial console modes not set". Inline startup must be
// able to turn capture off directly, preserving unrelated console mode bits.
pub(super) fn set_mouse_capture(out: &mut impl Write, enabled: bool) -> io::Result<()> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::{
            ENABLE_EXTENDED_FLAGS, ENABLE_MOUSE_INPUT, ENABLE_QUICK_EDIT_MODE, ENABLE_WINDOW_INPUT,
            GetConsoleMode, GetStdHandle, STD_INPUT_HANDLE, SetConsoleMode,
        };
        let _ = out;
        // SAFETY: the standard input handle is process-owned; mode points to a u32.
        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            let mut mode = 0;
            if GetConsoleMode(handle, &mut mode) == 0 {
                return Err(io::Error::last_os_error());
            }
            let next = if enabled {
                (mode | ENABLE_MOUSE_INPUT | ENABLE_WINDOW_INPUT | ENABLE_EXTENDED_FLAGS)
                    & !ENABLE_QUICK_EDIT_MODE
            } else {
                mode & !ENABLE_MOUSE_INPUT
            };
            if SetConsoleMode(handle, next) == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        if enabled {
            execute!(out, crossterm::event::EnableMouseCapture)
        } else {
            execute!(out, crossterm::event::DisableMouseCapture)
        }
    }
}
