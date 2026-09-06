//! Encode crossterm events for the child's logical terminal.
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use vt100::{MouseProtocolEncoding as Encoding, MouseProtocolMode as Mode};

pub fn key_bytes(key: KeyEvent, application_cursor: bool) -> Vec<u8> {
    if key.kind == KeyEventKind::Release {
        return Vec::new();
    }
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let modifier = 1 + u8::from(shift) + 2 * u8::from(alt) + 4 * u8::from(ctrl);
    let mut bytes = match key.code {
        KeyCode::Char(c) => {
            if ctrl && c.is_ascii() {
                match c {
                    ' ' | '@' | '2' => vec![0],
                    // Crossterm decodes legacy Ctrl+\\, ], ^, _ as Ctrl+4..7.
                    '4'..='7' => vec![c as u8 - b'4' + 0x1c],
                    'a'..='z' | 'A'..='Z' | '['..='_' => vec![c.to_ascii_uppercase() as u8 & 0x1f],
                    '?' | '8' => vec![127],
                    _ => c.to_string().into_bytes(),
                }
            } else {
                c.to_string().into_bytes()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Tab if shift => return b"\x1b[Z".to_vec(),
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => return b"\x1b[Z".to_vec(),
        KeyCode::Backspace => vec![if ctrl { 8 } else { 127 }],
        KeyCode::Esc => vec![27],
        KeyCode::Up
        | KeyCode::Down
        | KeyCode::Right
        | KeyCode::Left
        | KeyCode::Home
        | KeyCode::End => {
            let suffix = match key.code {
                KeyCode::Up => 'A',
                KeyCode::Down => 'B',
                KeyCode::Right => 'C',
                KeyCode::Left => 'D',
                KeyCode::Home => 'H',
                _ => 'F',
            };
            return if modifier > 1 {
                format!("\x1b[1;{modifier}{suffix}").into_bytes()
            } else if application_cursor {
                format!("\x1bO{suffix}").into_bytes()
            } else {
                format!("\x1b[{suffix}").into_bytes()
            };
        }
        KeyCode::Insert | KeyCode::Delete | KeyCode::PageUp | KeyCode::PageDown | KeyCode::F(_) => {
            let code = match key.code {
                KeyCode::Insert => 2,
                KeyCode::Delete => 3,
                KeyCode::PageUp => 5,
                KeyCode::PageDown => 6,
                KeyCode::F(n @ 1..=4) => {
                    let suffix = char::from(b'P' + n - 1);
                    return if modifier == 1 {
                        format!("\x1bO{suffix}").into_bytes()
                    } else {
                        format!("\x1b[1;{modifier}{suffix}").into_bytes()
                    };
                }
                KeyCode::F(n @ 5..=12) => [15, 17, 18, 19, 20, 21, 23, 24][usize::from(n - 5)],
                _ => return Vec::new(),
            };
            return if modifier == 1 {
                format!("\x1b[{code}~").into_bytes()
            } else {
                format!("\x1b[{code};{modifier}~").into_bytes()
            };
        }
        _ => return Vec::new(),
    };
    if alt {
        bytes.insert(0, 27);
    }
    bytes
}

pub fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    // Preserve the text itself exactly. Bracketing belongs to the child's mode.
    if bracketed {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.as_bytes().to_vec()
    }
}

pub fn mouse_bytes(event: MouseEvent, mode: Mode, encoding: Encoding, logical_col: u16) -> Vec<u8> {
    if mode == Mode::None {
        return Vec::new();
    }
    let button = |b| match b {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    };
    let mut code = match event.kind {
        MouseEventKind::Down(b) => button(b),
        MouseEventKind::Up(_) if mode == Mode::Press => return Vec::new(),
        MouseEventKind::Up(b) => {
            if encoding == Encoding::Sgr {
                button(b)
            } else {
                3
            }
        }
        MouseEventKind::Drag(b) if matches!(mode, Mode::ButtonMotion | Mode::AnyMotion) => {
            32 + button(b)
        }
        MouseEventKind::Moved if mode == Mode::AnyMotion => 35,
        MouseEventKind::ScrollUp => 64,
        MouseEventKind::ScrollDown => 65,
        MouseEventKind::ScrollLeft => 66,
        MouseEventKind::ScrollRight => 67,
        _ => return Vec::new(),
    };
    if event.modifiers.contains(KeyModifiers::SHIFT) {
        code += 4;
    }
    if event.modifiers.contains(KeyModifiers::ALT) {
        code += 8;
    }
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        code += 16;
    }
    let x = u32::from(logical_col) + 1;
    let y = u32::from(event.row) + 1;
    if encoding == Encoding::Sgr {
        let suffix = if matches!(event.kind, MouseEventKind::Up(_)) {
            'm'
        } else {
            'M'
        };
        return format!("\x1b[<{code};{x};{y}{suffix}").into_bytes();
    }
    let mut bytes = b"\x1b[M".to_vec();
    for value in [code + 32, x + 32, y + 32] {
        if encoding == Encoding::Utf8 {
            if let Some(c) = char::from_u32(value) {
                bytes.extend_from_slice(c.to_string().as_bytes());
            } else {
                return Vec::new();
            }
        } else if value <= 255 {
            bytes.push(value as u8);
        } else {
            return Vec::new();
        }
    }
    bytes
}
