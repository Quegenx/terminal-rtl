//! Host input keeps native Windows key records intact for the nested ConPTY.
use crossterm::event::Event;
#[cfg(not(windows))]
use std::{io, time::Duration};

pub(super) struct HostInput {
    pub event: Event,
    pub native_key: Option<Vec<u8>>,
}

#[cfg(windows)]
pub(super) use super::windows_input::HostInputReader;

#[cfg(not(windows))]
pub(super) struct HostInputReader;

#[cfg(not(windows))]
impl HostInputReader {
    pub(super) fn new() -> Self {
        Self
    }

    pub(super) fn read(&mut self, timeout: Duration) -> io::Result<Option<HostInput>> {
        if !crossterm::event::poll(timeout)? {
            return Ok(None);
        }
        Ok(Some(HostInput {
            event: crossterm::event::read()?,
            native_key: None,
        }))
    }
}
