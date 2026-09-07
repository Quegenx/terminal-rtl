//! Decode ConPTY's win32-input-mode envelope and route host VT mouse/history keys.
use super::{input_reader::HostInput, windows_mouse::sgr_mouse_event};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use windows_sys::Win32::System::Console::KEY_EVENT_RECORD;

pub(super) enum ConsoleSequence {
    Key(KEY_EVENT_RECORD),
    Input(HostInput),
    Paste,
}

pub(super) fn decode_console_sequence(units: &[u16]) -> ConsoleSequence {
    let sequence = String::from_utf16_lossy(units);
    let raw = sequence.as_bytes().to_vec();
    let fallback = || {
        ConsoleSequence::Input(HostInput {
            event: Event::Key(KeyEvent::new(KeyCode::Null, KeyModifiers::NONE)),
            native_key: Some(raw.clone()),
        })
    };
    let Some(body) = sequence.strip_prefix("\x1b[") else {
        return fallback();
    };
    let Some(final_byte) = body.as_bytes().last().copied() else {
        return fallback();
    };
    let Some(parameters) = body.get(..body.len() - 1) else {
        return fallback();
    };
    if matches!(final_byte, b'M' | b'm') && parameters.starts_with('<') {
        if let Some(event) = sgr_mouse_event(&parameters[1..], final_byte == b'm') {
            return ConsoleSequence::Input(HostInput {
                event,
                native_key: None,
            });
        }
        return fallback();
    }
    let Ok(params) = parameters
        .split(';')
        .map(|p| {
            if p.is_empty() {
                Ok(0)
            } else {
                p.parse::<u16>()
            }
        })
        .collect::<Result<Vec<_>, _>>()
    else {
        return fallback();
    };
    let param = |index: usize| params.get(index).copied().unwrap_or(0);
    if final_byte == b'_' && params.len() <= 6 {
        return ConsoleSequence::Key(KEY_EVENT_RECORD {
            wVirtualKeyCode: param(0),
            wVirtualScanCode: param(1),
            uChar: windows_sys::Win32::System::Console::KEY_EVENT_RECORD_0 {
                UnicodeChar: param(2),
            },
            bKeyDown: i32::from(param(3) != 0),
            dwControlKeyState: u32::from(param(4)),
            wRepeatCount: param(5).max(1),
        });
    }
    if final_byte == b'~' && param(0) == 200 {
        return ConsoleSequence::Paste;
    }
    let modifier = param(1).saturating_sub(1);
    let mut modifiers = KeyModifiers::NONE;
    if modifier & 1 != 0 {
        modifiers |= KeyModifiers::SHIFT;
    }
    if modifier & 2 != 0 {
        modifiers |= KeyModifiers::ALT;
    }
    if modifier & 4 != 0 {
        modifiers |= KeyModifiers::CONTROL;
    }
    if final_byte == b'u' && param(0) == 13 {
        return ConsoleSequence::Key(KEY_EVENT_RECORD {
            wVirtualKeyCode: 13,
            wVirtualScanCode: 28,
            uChar: windows_sys::Win32::System::Console::KEY_EVENT_RECORD_0 { UnicodeChar: 13 },
            bKeyDown: 1,
            wRepeatCount: 1,
            dwControlKeyState: u32::from(modifier & 1) * 16
                + u32::from(modifier & 2)
                + u32::from(modifier & 4) * 2,
        });
    }
    let code = match (final_byte, param(0)) {
        (b'~', 5) => KeyCode::PageUp,
        (b'~', 6) => KeyCode::PageDown,
        _ => return fallback(),
    };
    ConsoleSequence::Input(HostInput {
        event: Event::Key(KeyEvent::new(code, modifiers)),
        native_key: Some(raw),
    })
}
