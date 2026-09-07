use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use terminal_rtl::{
    display::Renderer,
    input::{key_bytes, mouse_bytes, paste_bytes},
    protocol::Protocol,
};

use super::input_reader::HostInput;
use crate::Args;

pub(super) enum InputAction {
    Ignore,
    Redraw,
    Resize(u16, u16),
    Quit,
    Send { bytes: Vec<u8>, redraw: bool },
}

pub(super) struct SessionInput {
    pub enabled: bool,
    pub scrollback: usize,
    prefix: bool,
    history_limit: usize,
}

impl SessionInput {
    pub(super) fn new(args: &Args) -> Self {
        Self {
            enabled: !args.no_bidi,
            scrollback: 0,
            prefix: false,
            history_limit: usize::from(args.scrollback),
        }
    }

    pub(super) fn handle(
        &mut self,
        host_input: HostInput,
        parser: &vt100::Parser<Protocol>,
        renderer: &Renderer,
    ) -> InputAction {
        let HostInput { event, native_key } = host_input;
        let encode_key = |key| {
            native_key
                .clone()
                .unwrap_or_else(|| key_bytes(key, parser.screen().application_cursor()))
        };
        let screen = parser.screen();
        let mut redraw = false;
        let bytes = match event {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Release {
                    return InputAction::Ignore;
                }
                let is_prefix = key_bytes(key, false) == [0x1d];
                if self.prefix {
                    self.prefix = false;
                    match key.code {
                        KeyCode::Char('r') => {
                            self.enabled = !self.enabled;
                            return InputAction::Redraw;
                        }
                        KeyCode::Char('q') => return InputAction::Quit,
                        _ => {
                            let mut bytes = if is_prefix { Vec::new() } else { vec![0x1d] };
                            bytes.extend(encode_key(key));
                            bytes
                        }
                    }
                } else if is_prefix {
                    self.prefix = true;
                    return InputAction::Ignore;
                } else if key.modifiers.contains(KeyModifiers::SHIFT)
                    && matches!(key.code, KeyCode::PageUp | KeyCode::PageDown)
                {
                    let step = usize::from(screen.size().0.saturating_sub(1).max(1));
                    self.scrollback = if key.code == KeyCode::PageUp {
                        self.scrollback.saturating_add(step).min(self.history_limit)
                    } else {
                        self.scrollback.saturating_sub(step)
                    };
                    return InputAction::Redraw;
                } else {
                    if self.scrollback > 0 {
                        self.scrollback = 0;
                        redraw = true;
                    }
                    encode_key(key)
                }
            }
            Event::Paste(text) => {
                self.scrollback = 0;
                redraw = true;
                paste_bytes(&text, screen.bracketed_paste())
            }
            Event::Resize(cols, rows) => return InputAction::Resize(cols, rows),
            Event::FocusGained if parser.callbacks().focus_events => b"\x1b[I".to_vec(),
            Event::FocusLost if parser.callbacks().focus_events => b"\x1b[O".to_vec(),
            Event::Mouse(mouse)
                if matches!(
                    mouse.kind,
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                ) && (self.scrollback > 0
                    || screen.mouse_protocol_mode() == vt100::MouseProtocolMode::None) =>
            {
                // Browsing never sends prompt-history keys into the child.
                if screen.alternate_screen() {
                    return InputAction::Ignore;
                }
                self.scrollback = if mouse.kind == MouseEventKind::ScrollUp {
                    self.scrollback.saturating_add(3).min(self.history_limit)
                } else {
                    self.scrollback.saturating_sub(3)
                };
                return InputAction::Redraw;
            }
            Event::Mouse(mouse)
                if self.scrollback == 0
                    && mouse.column >= renderer.margin()
                    && mouse.row < screen.size().0 =>
            {
                mouse_bytes(
                    mouse,
                    screen.mouse_protocol_mode(),
                    screen.mouse_protocol_encoding(),
                    renderer.logical_column(mouse.row, mouse.column),
                )
            }
            _ => return InputAction::Ignore,
        };
        InputAction::Send { bytes, redraw }
    }
}
