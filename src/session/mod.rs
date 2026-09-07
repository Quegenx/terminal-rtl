//! Own the child and host terminal; the engine coordinates output and input.
mod command;
mod engine;
mod host;
mod interaction;
mod output;
mod shutdown;

use anyhow::{Context, Result};
use crossterm::terminal;
use portable_pty::{Child, native_pty_system};
use std::{
    fs::File,
    io::{self, Write},
};

use crate::Args;
use command::child_command;
use host::{TerminalGuard, child_geometry};

pub(super) struct ChildGuard {
    pub(super) child: Box<dyn Child + Send + Sync>,
    pub(super) exited: bool,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.exited {
            let _ = self.child.kill();
        }
    }
}

pub fn run(args: &Args, recording: Option<File>) -> Result<u32> {
    let shutdown = shutdown::Shutdown::install().context("installing shutdown handler")?;
    let (cols, rows) = terminal::size().context("cannot read terminal size")?;
    let initial = child_geometry(args, cols, rows).0;
    let pair = native_pty_system()
        .openpty(initial)
        .context("cannot create pseudo-terminal")?;
    let reader = pair.master.try_clone_reader()?;
    let writer = pair.master.take_writer()?;
    let child = pair
        .slave
        .spawn_command(child_command(&args.command)?)
        .with_context(|| format!("cannot launch {}", args.command[0].to_string_lossy()))?;
    let mut child = ChildGuard {
        child,
        exited: false,
    };
    drop(pair.slave);

    let receive = output::read_child_output(reader);
    let guard = TerminalGuard::enter(args.inline)?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine::TerminalSession {
            args,
            master: pair.master.as_ref(),
            child: &mut child,
            shutdown: &shutdown,
            receive,
            writer,
            recording,
        }
        .run((cols, rows))
    }));
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
