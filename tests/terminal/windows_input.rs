use super::*;

#[test]
fn conpty_envelopes_preserve_fragmented_paste_and_mouse_events() {
    let mut reader = HostInputReader::new();
    let paste = "\x1b[200~שלום 😀\x1b[200~inside\x1dquit\x1b[201~";
    for unit in paste.encode_utf16() {
        let envelope = format!("\x1b[0;0;{unit};1;0;1_");
        for ascii in envelope.encode_utf16() {
            reader.accept_key(text_key(ascii));
        }
    }
    let input = reader.ready.pop_front().unwrap();
    assert_eq!(
        input.event,
        Event::Paste("שלום 😀\x1b[200~inside\x1dquit".into())
    );
    assert!(reader.ready.is_empty());
    for unit in "\x1b[<64;4;5M\x1b[<65;4;5M".encode_utf16() {
        for ascii in format!("\x1b[0;0;{unit};1;0;1_").encode_utf16() {
            reader.accept_key(text_key(ascii));
        }
    }
    for kind in [
        crossterm::event::MouseEventKind::ScrollUp,
        crossterm::event::MouseEventKind::ScrollDown,
    ] {
        let Event::Mouse(mouse) = reader.ready.pop_front().unwrap().event else {
            panic!("expected mouse");
        };
        assert_eq!(mouse.kind, kind);
        assert_eq!((mouse.column, mouse.row), (3, 4));
    }
    assert!(reader.ready.is_empty());
}
