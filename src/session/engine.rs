use std::{
    fs::File,
    io::{self, Write},
    sync::mpsc::{Receiver, TryRecvError},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use crossterm::terminal;
use portable_pty::MasterPty;
use terminal_rtl::{display::Renderer, protocol::Protocol};

use super::{
    ChildGuard,
    host::{child_geometry, configure_renderer, resize, set_mouse_capture},
    input_reader::HostInputReader,
    interaction::{InputAction, SessionInput},
    output::ChildOutput,
    shutdown::Shutdown,
};
use crate::Args;

pub(super) struct TerminalSession<'a> {
    pub args: &'a Args,
    pub master: &'a (dyn MasterPty + Send),
    pub child: &'a mut ChildGuard,
    pub shutdown: &'a Shutdown,
    pub receive: Receiver<ChildOutput>,
    pub writer: Box<dyn Write + Send>,
    pub recording: Option<File>,
}

impl TerminalSession<'_> {
    pub(super) fn run(self, mut current_size: (u16, u16)) -> Result<(u32, Vec<u8>)> {
        let Self {
            args,
            master,
            child,
            shutdown,
            receive,
            mut writer,
            mut recording,
        } = self;
        let initial = child_geometry(args, current_size.0, current_size.1).0;
        let snapshot = |screen: &vt100::Screen, enabled| {
            if args.no_replay
                || args.inline
                || (cfg!(windows) && terminal::size().is_ok_and(|(cols, _)| cols < 2))
            {
                Vec::new()
            } else {
                terminal_rtl::display::replay(screen, enabled, args.direction, args.layout)
            }
        };
        let mut parser = vt100::Parser::new_with_callbacks(
            initial.rows,
            initial.cols,
            usize::from(args.scrollback),
            Protocol::default(),
        );
        let mut renderer = Renderer::default();
        renderer.set_pretty(args.pretty);
        renderer.set_layout(args.layout);
        configure_renderer(&mut renderer, args, current_size.0, current_size.1);
        let mut input = SessionInput::new(args);
        let mut host_input = HostInputReader::new();
        let mut mouse_capture = !args.inline;
        let mut dirty = true;
        let mut eof = false;
        let mut exit = None;
        let mut exit_time = None;
        let mut shutdown_time = None;
        let mut pipe_error = None;
        let mut last_render = Instant::now() - Duration::from_secs(1);
        let mut sync_started = None;

        let mut last_size_check = Instant::now();
        let mut out = io::stdout();
        loop {
            if let Some(code) = shutdown.exit_code()
                && shutdown_time.is_none()
            {
                child.child.kill().context("terminating child")?;
                shutdown_time = Some((Instant::now(), code));
            }
            if let Some((started, code)) = shutdown_time
                && (eof || started.elapsed() >= Duration::from_millis(500))
            {
                return Ok((code, snapshot(parser.screen(), input.enabled)));
            }
            // A resize signal/event can be coalesced or missed by a host. The
            // occasional size query also covers nested PTYs and ConPTY hosts.
            if cfg!(windows) || last_size_check.elapsed() >= Duration::from_millis(200) {
                last_size_check = Instant::now();
                if let Ok((cols, rows)) = terminal::size()
                    && (cols, rows) != current_size
                {
                    resize(master, &mut parser, &mut renderer, args, (cols, rows))?;
                    current_size = (cols, rows);
                    dirty = true;
                }
            }
            // Time-slice output so continuous streaming cannot starve input.
            let output_slice = Instant::now();
            for _ in 0..64 {
                if output_slice.elapsed() >= Duration::from_millis(8) {
                    break;
                }
                match receive.try_recv() {
                    Ok(ChildOutput::Data(bytes)) => {
                        if let Some(file) = &mut recording {
                            file.write_all(&bytes).context("recording failed")?;
                        }
                        if args.inline {
                            parser.process_with_history(&bytes, |rows| {
                                renderer.append_history(
                                    rows,
                                    current_size,
                                    input.enabled,
                                    args.direction,
                                    &mut out,
                                )
                            })?;
                        } else {
                            parser.process(&bytes);
                        }
                        dirty = true;
                        if !parser.callbacks().replies.is_empty() {
                            let replies = std::mem::take(&mut parser.callbacks_mut().replies);
                            writer.write_all(&replies)?;
                            writer.flush()?;
                        }
                    }
                    Err(TryRecvError::Disconnected) => {
                        eof = true;
                        break;
                    }
                    Ok(ChildOutput::Error(e))
                        if cfg!(windows) && matches!(e.raw_os_error(), Some(109 | 232)) =>
                    {
                        // Broken/closing ConPTY pipe may precede try_wait.
                        pipe_error = Some((Instant::now(), e));
                        eof = true;
                        break;
                    }
                    Ok(ChildOutput::Error(e)) => return Err(e).context("reading agent output"),
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
            let sync_ready = sync_started.is_none_or(|t| t.elapsed() >= Duration::from_millis(250));
            if dirty
                && (!cfg!(windows) || current_size.0 >= 2)
                && (exit.is_some()
                    || (sync_ready && last_render.elapsed() >= Duration::from_millis(16)))
            {
                // Never leave the live parser in scrollback while processing bytes.
                if input.scrollback > 0 && !parser.screen().alternate_screen() {
                    input.scrollback = input.scrollback.min(parser.screen().retained_rows());
                    renderer.render_scrollback(
                        parser.screen(),
                        input.scrollback,
                        input.enabled,
                        args.direction,
                        &mut out,
                    )?;
                } else {
                    input.scrollback = 0;
                    renderer.render(parser.screen(), input.enabled, args.direction, &mut out)?;
                }
                last_render = Instant::now();
                dirty = false;
            }
            let wants_mouse = !args.inline
                || (input.scrollback == 0
                    && parser.screen().mouse_protocol_mode() != vt100::MouseProtocolMode::None);
            if wants_mouse != mouse_capture {
                set_mouse_capture(&mut out, wants_mouse)?;
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
            if exit.is_none()
                && pipe_error
                    .as_ref()
                    .is_some_and(|(t, _)| t.elapsed() >= Duration::from_millis(500))
            {
                return Err(pipe_error.take().unwrap().1)
                    .context("reading agent output before child exit");
            }
            // Drain final PTY output. ConPTY may keep a pipe open after child exit.
            if let Some(code) = exit
                && (eof || exit_time.is_some_and(|t| t.elapsed() >= Duration::from_millis(500)))
            {
                return Ok((
                    shutdown_time.map_or(code, |(_, code)| code),
                    snapshot(parser.screen(), input.enabled),
                ));
            }

            let Some(event) = host_input.read(Duration::from_millis(4))? else {
                continue;
            };
            match input.handle(event, &parser, &renderer) {
                InputAction::Ignore => {}
                InputAction::Redraw => dirty = true,
                InputAction::Resize(cols, rows) => {
                    resize(master, &mut parser, &mut renderer, args, (cols, rows))?;
                    current_size = (cols, rows);
                    dirty = true;
                }
                InputAction::Quit => {
                    child.child.kill()?;
                    return Ok((130, snapshot(parser.screen(), input.enabled)));
                }
                InputAction::Send { bytes, redraw } => {
                    dirty |= redraw;
                    if !bytes.is_empty() && exit.is_none() && shutdown_time.is_none() {
                        writer.write_all(&bytes).context("sending input to agent")?;
                        writer.flush()?;
                    }
                }
            }
        }
    }
}
