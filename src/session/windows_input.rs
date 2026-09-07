//! Native console records plus VT input preserve both modifiers and paste framing.
use super::{
    input_reader::HostInput,
    windows_mouse::ConsoleMouse,
    windows_sequence::{ConsoleSequence, decode_console_sequence},
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::{
    collections::VecDeque,
    io, thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::System::Console::*;

pub(super) struct HostInputReader {
    ready: VecDeque<HostInput>,
    opener: Vec<u16>,
    paste: Option<Vec<u16>>,
    last_key: Instant,
    mouse: ConsoleMouse,
}

impl HostInputReader {
    pub(super) fn new() -> Self {
        Self {
            ready: VecDeque::new(),
            opener: Vec::new(),
            paste: None,
            last_key: Instant::now(),
            mouse: ConsoleMouse::default(),
        }
    }

    pub(super) fn read(&mut self, timeout: Duration) -> io::Result<Option<HostInput>> {
        if let Some(event) = self.ready.pop_front() {
            return Ok(Some(event));
        }
        // This session is the sole console-input reader. Read only queued records
        // so child output, process exit and shutdown remain serviced while idle.
        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            let mut count = 0;
            if GetNumberOfConsoleInputEvents(handle, &mut count) == 0 {
                return Err(io::Error::last_os_error());
            }
            if count > 0 {
                let mut records = [INPUT_RECORD::default(); 64];
                let length = count.min(records.len() as u32);
                if ReadConsoleInputW(handle, records.as_mut_ptr(), length, &mut count) == 0 {
                    return Err(io::Error::last_os_error());
                }
                for record in &records[..count as usize] {
                    match u32::from(record.EventType) {
                        KEY_EVENT => self.accept_key(record.Event.KeyEvent),
                        MOUSE_EVENT => {
                            if let Some(event) = self.mouse.decode(record.Event.MouseEvent)? {
                                self.ready.push_back(HostInput {
                                    event,
                                    native_key: None,
                                });
                            }
                        }
                        WINDOW_BUFFER_SIZE_EVENT => {
                            let (cols, rows) = crossterm::terminal::size()?;
                            self.ready.push_back(HostInput {
                                event: Event::Resize(cols, rows),
                                native_key: None,
                            });
                        }
                        FOCUS_EVENT => self.ready.push_back(HostInput {
                            event: if record.Event.FocusEvent.bSetFocus != 0 {
                                Event::FocusGained
                            } else {
                                Event::FocusLost
                            },
                            native_key: None,
                        }),
                        _ => {}
                    }
                }
            }
        }
        // Only a lone Escape is ambiguous with an Escape key press. A partial
        // CSI can span console reads and must survive arbitrary scheduling delays.
        if self.opener == [27] && self.last_key.elapsed() >= Duration::from_millis(100) {
            self.flush_opener();
        }
        let event = self.ready.pop_front();
        if event.is_none() {
            thread::sleep(timeout);
        }
        Ok(event)
    }

    fn accept_key(&mut self, key: KEY_EVENT_RECORD) {
        // Ignore release events as on the existing crossterm input path, except
        // Alt-code releases, whose UnicodeChar is the actual typed character.
        let alt_code =
            key.wVirtualKeyCode == 18 && key.bKeyDown == 0 && unsafe { key.uChar.UnicodeChar } != 0;
        if key.bKeyDown == 0 && !alt_code {
            return;
        }
        let unit = unsafe { key.uChar.UnicodeChar };
        if matches!(key.wVirtualKeyCode, 16..=18) && unit == 0 {
            return; // Modifier transitions must not consume the Ctrl+] prefix.
        }
        if key.wVirtualKeyCode != 0 {
            self.ready.push_back(native_key_input(key));
            return;
        }
        self.last_key = Instant::now();
        if let Some(text) = &mut self.paste {
            text.extend(std::iter::repeat_n(
                unit,
                usize::from(key.wRepeatCount.max(1)),
            ));
            if text.ends_with(&[27, 91, 50, 48, 49, 126]) {
                text.truncate(text.len() - 6);
                let text = String::from_utf16_lossy(text);
                self.paste = None;
                self.ready.push_back(HostInput {
                    event: Event::Paste(text),
                    native_key: None,
                });
            }
            return;
        }
        if self.opener.is_empty() {
            if unit == 27 {
                self.opener.push(unit);
            } else {
                self.ready.push_back(native_key_input(key));
            }
            return;
        }
        self.opener.push(unit);
        let complete = if self.opener.get(1) == Some(&91) {
            self.opener.len() > 2 && (64..=126).contains(&unit)
        } else {
            self.opener.len() > 2 || unit != 79
        };
        if complete || self.opener.len() >= 128 {
            let sequence = std::mem::take(&mut self.opener);
            match decode_console_sequence(&sequence) {
                ConsoleSequence::Key(key) => {
                    // Decoded native key records are already past the VT layer.
                    if key.bKeyDown != 0
                        && !(matches!(key.wVirtualKeyCode, 16..=18)
                            && unsafe { key.uChar.UnicodeChar } == 0)
                    {
                        self.ready.push_back(native_key_input(key));
                    }
                }
                ConsoleSequence::Input(input) => self.ready.push_back(input),
                ConsoleSequence::Paste => self.paste = Some(Vec::new()),
            }
        }
    }

    fn flush_opener(&mut self) {
        if !self.opener.is_empty() {
            self.ready.push_back(HostInput {
                event: Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
                native_key: Some(
                    String::from_utf16_lossy(&std::mem::take(&mut self.opener)).into_bytes(),
                ),
            });
        }
    }
}

pub(super) fn console_modifiers(state: u32) -> KeyModifiers {
    let mut modifiers = KeyModifiers::empty();
    if state & SHIFT_PRESSED != 0 {
        modifiers |= KeyModifiers::SHIFT;
    }
    if state & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED) != 0 {
        modifiers |= KeyModifiers::ALT;
    }
    if state & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED) != 0 {
        modifiers |= KeyModifiers::CONTROL;
    }
    modifiers
}

fn native_key_input(key: KEY_EVENT_RECORD) -> HostInput {
    let unit = unsafe { key.uChar.UnicodeChar };
    // Decode only the keys interpreted by SessionInput. Every other key is sent
    // with its original VK, scan code, Unicode unit, modifiers and repeat count.
    let code = match (key.wVirtualKeyCode, unit) {
        (_, 29) => KeyCode::Char(']'),
        (33, _) => KeyCode::PageUp,
        (34, _) => KeyCode::PageDown,
        (_, unit) => char::from_u32(u32::from(unit)).map_or(KeyCode::Null, KeyCode::Char),
    };
    let mut modifiers = console_modifiers(key.dwControlKeyState);
    if unit == 29 {
        modifiers |= KeyModifiers::CONTROL;
    }
    let event = KeyEvent::new_with_kind(code, modifiers, KeyEventKind::Press);
    // Microsoft win32-input-mode: CSI Vk;Sc;Uc;Kd;Cs;Rc _. portable-pty's Windows
    // master is always ConPTY, which requests and accepts this native encoding.
    let native = format!(
        "\x1b[{};{};{};{};{};{}_",
        key.wVirtualKeyCode,
        key.wVirtualScanCode,
        unit,
        key.bKeyDown,
        key.dwControlKeyState,
        key.wRepeatCount
    );
    HostInput {
        event: Event::Key(event),
        native_key: Some(native.into_bytes()),
    }
}
