//! Answer terminal queries from logical state, never from the reordered host.
#[derive(Default)]
pub struct Protocol {
    pub replies: Vec<u8>,
    pub bell: bool,
    pub clear_scrollback: bool,
    pub focus_events: bool,
    pub synchronized_output: bool,
    hyperlink_serial: u64,
}

impl vt100::Callbacks for Protocol {
    fn audible_bell(&mut self, _: &mut vt100::Screen) {
        self.bell = true;
    }

    fn unhandled_escape(&mut self, _: &mut vt100::Screen, i1: Option<u8>, _: Option<u8>, c: u8) {
        if i1.is_none() && c == b'Z' {
            self.replies.extend_from_slice(b"\x1b[?1;2c");
        }
    }

    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        i1: Option<u8>,
        i2: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        let first = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        match (i1, i2, c, first) {
            (None, None, 'J', 3) => {
                screen.clear_scrollback();
                self.clear_scrollback = true;
            }
            (None, None, 'n', 5) => self.replies.extend_from_slice(b"\x1b[0n"),
            (None | Some(b'?'), None, 'n', 6) => {
                let (row, col) = screen.cursor_position();
                let col = col.min(screen.size().1 - 1);
                let private = if i1.is_some() { "?" } else { "" };
                self.replies.extend_from_slice(
                    format!("\x1b[{private}{};{}R", row + 1, col + 1).as_bytes(),
                );
            }
            (None, None, 'c', 0) => self.replies.extend_from_slice(b"\x1b[?1;2c"),
            (Some(b'>'), None, 'c', 0) => self.replies.extend_from_slice(b"\x1b[>0;100;0c"),
            (Some(b'?'), None, 'u', _) => self.replies.extend_from_slice(b"\x1b[?0u"),
            (None, None, 't', 18) => {
                let (rows, cols) = screen.size();
                self.replies
                    .extend_from_slice(format!("\x1b[8;{rows};{cols}t").as_bytes());
            }
            (Some(b'?'), None, 'h' | 'l', _) => {
                for value in params.iter().filter_map(|p| p.first()) {
                    match *value {
                        1004 => self.focus_events = c == 'h',
                        2026 => self.synchronized_output = c == 'h',
                        _ => {}
                    }
                }
            }
            (Some(b'?'), Some(b'$'), 'p', mode) => {
                let state = match mode {
                    1 => Some(screen.application_cursor()),
                    25 => Some(!screen.hide_cursor()),
                    1004 => Some(self.focus_events),
                    1049 => Some(screen.alternate_screen()),
                    2004 => Some(screen.bracketed_paste()),
                    2026 => Some(self.synchronized_output),
                    _ => None,
                };
                let value = state.map_or(0, |v| if v { 1 } else { 2 });
                self.replies
                    .extend_from_slice(format!("\x1b[?{mode};{value}$y").as_bytes());
            }
            _ => {}
        }
    }

    // Give a deterministic dark palette rather than forwarding queries whose
    // replies crossterm cannot transport. Unsupported OSCs are deliberately not
    // written raw into the host renderer.
    fn unhandled_osc(&mut self, screen: &mut vt100::Screen, params: &[&[u8]]) {
        if params.first() == Some(&b"8".as_slice()) {
            // vte splits every semicolon, including those inside a destination.
            // Reassemble only a bounded URI and never forward unvalidated OSCs.
            screen.set_hyperlink(None);
            if params.len() < 3 || params.iter().map(|p| p.len() + 1).sum::<usize>() > 9216 {
                return;
            }
            let Ok(uri) = String::from_utf8(params[2..].join(&b';')) else {
                return;
            };
            let Ok(parameters) = std::str::from_utf8(params[1]) else {
                return;
            };
            self.hyperlink_serial = self.hyperlink_serial.wrapping_add(1);
            let id = if let Some(id) = parameters
                .split(':')
                .find_map(|p| p.strip_prefix("id="))
                .filter(|id| !id.is_empty())
            {
                format!("rtl-{}-id-{id}", std::process::id())
            } else {
                format!("rtl-{}-auto-{}", std::process::id(), self.hyperlink_serial)
            };
            screen.set_hyperlink(vt100::Hyperlink::new(uri, id).map(std::sync::Arc::new));
            return;
        }
        if params.get(1) != Some(&b"?".as_slice()) {
            return;
        }
        let (number, color) = match params.first().copied() {
            Some(b"10") => ("10", "dddd/dddd/dddd"),
            Some(b"11") => ("11", "1111/1111/1111"),
            _ => return,
        };
        self.replies
            .extend_from_slice(format!("\x1b]{number};rgb:{color}\x1b\\").as_bytes());
    }
}
