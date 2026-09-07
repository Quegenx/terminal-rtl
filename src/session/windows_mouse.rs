//! Convert native mouse coordinates and button transitions for logical routing.
use super::windows_input::console_modifiers;
use crossterm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use std::io;
use windows_sys::Win32::System::Console::*;

#[derive(Default)]
pub(super) struct ConsoleMouse {
    buttons: u32,
}

impl ConsoleMouse {
    pub(super) fn decode(&mut self, mouse: MOUSE_EVENT_RECORD) -> io::Result<Option<Event>> {
        let buttons = mouse.dwButtonState & 0xffff;
        let wheel = (mouse.dwButtonState >> 16) as i16;
        let button = |mask| match mask {
            1 => MouseButton::Left,
            2 => MouseButton::Right,
            _ => MouseButton::Middle,
        };
        let kind = match mouse.dwEventFlags {
            MOUSE_WHEELED if wheel > 0 => Some(MouseEventKind::ScrollUp),
            MOUSE_WHEELED if wheel < 0 => Some(MouseEventKind::ScrollDown),
            MOUSE_HWHEELED if wheel > 0 => Some(MouseEventKind::ScrollRight),
            MOUSE_HWHEELED if wheel < 0 => Some(MouseEventKind::ScrollLeft),
            MOUSE_MOVED => Some(
                match [1, 2, 4].into_iter().find(|mask| buttons & mask != 0) {
                    Some(mask) => MouseEventKind::Drag(button(mask)),
                    None => MouseEventKind::Moved,
                },
            ),
            0 | DOUBLE_CLICK => [1, 2, 4]
                .into_iter()
                .find(|mask| (buttons ^ self.buttons) & mask != 0)
                .map(|mask| {
                    if buttons & mask != 0 {
                        MouseEventKind::Down(button(mask))
                    } else {
                        MouseEventKind::Up(button(mask))
                    }
                }),
            _ => None,
        };
        self.buttons = buttons;
        let Some(kind) = kind else {
            return Ok(None);
        };
        let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
        // SAFETY: the standard output handle is borrowed; info is writable.
        unsafe {
            if GetConsoleScreenBufferInfo(GetStdHandle(STD_OUTPUT_HANDLE), &mut info) == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        let column = mouse.dwMousePosition.X - info.srWindow.Left;
        let row = mouse.dwMousePosition.Y - info.srWindow.Top;
        if column < 0 || row < 0 {
            return Ok(None);
        }
        Ok(Some(Event::Mouse(MouseEvent {
            kind,
            column: column as u16,
            row: row as u16,
            modifiers: console_modifiers(mouse.dwControlKeyState),
        })))
    }
}

pub(super) fn sgr_mouse_event(parameters: &str, release: bool) -> Option<Event> {
    let values = parameters
        .split(';')
        .map(str::parse::<u16>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let [code, x, y] = values.as_slice() else {
        return None;
    };
    let button = match code & 3 {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        _ => MouseButton::Right,
    };
    let kind = if code & 64 != 0 {
        match code & 3 {
            0 => MouseEventKind::ScrollUp,
            1 => MouseEventKind::ScrollDown,
            2 => MouseEventKind::ScrollLeft,
            _ => MouseEventKind::ScrollRight,
        }
    } else if release {
        MouseEventKind::Up(button)
    } else if code & 32 != 0 {
        if code & 3 == 3 {
            MouseEventKind::Moved
        } else {
            MouseEventKind::Drag(button)
        }
    } else {
        MouseEventKind::Down(button)
    };
    let mut modifiers = crossterm::event::KeyModifiers::NONE;
    if code & 4 != 0 {
        modifiers |= crossterm::event::KeyModifiers::SHIFT;
    }
    if code & 8 != 0 {
        modifiers |= crossterm::event::KeyModifiers::ALT;
    }
    if code & 16 != 0 {
        modifiers |= crossterm::event::KeyModifiers::CONTROL;
    }
    Some(Event::Mouse(MouseEvent {
        kind,
        column: x.checked_sub(1)?,
        row: y.checked_sub(1)?,
        modifiers,
    }))
}
