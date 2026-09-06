use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers as Mod, MouseButton, MouseEvent, MouseEventKind,
};
use terminal_rtl::{
    input::{key_bytes, mouse_bytes, paste_bytes},
    protocol::Protocol,
};

#[test]
fn input_preserves_hebrew_and_control_keys() {
    assert_eq!(
        key_bytes(KeyEvent::new(KeyCode::Char('ש'), Mod::NONE), false),
        "ש".as_bytes()
    );
    assert_eq!(
        key_bytes(KeyEvent::new(KeyCode::Char('c'), Mod::CONTROL), false),
        [3]
    );
    assert_eq!(
        key_bytes(KeyEvent::new(KeyCode::Char('b'), Mod::ALT), false),
        b"\x1bb"
    );
    assert_eq!(
        key_bytes(KeyEvent::new(KeyCode::Up, Mod::NONE), true),
        b"\x1bOA"
    );
    assert_eq!(
        key_bytes(KeyEvent::new(KeyCode::Left, Mod::CONTROL), false),
        b"\x1b[1;5D"
    );
    assert!(
        key_bytes(
            KeyEvent::new_with_kind(KeyCode::Char('a'), Mod::NONE, KeyEventKind::Release),
            false
        )
        .is_empty()
    );
}

#[test]
fn paste_uses_the_childs_mode_and_preserves_original_order() {
    assert_eq!(paste_bytes("שלום\nworld", false), "שלום\nworld".as_bytes());
    assert_eq!(
        paste_bytes("שלום\nworld", true),
        "\x1b[200~שלום\nworld\x1b[201~".as_bytes()
    );
}

#[test]
fn mouse_reports_logical_coordinates() {
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 3,
        modifiers: Mod::NONE,
    };
    assert_eq!(
        mouse_bytes(
            event,
            vt100::MouseProtocolMode::PressRelease,
            vt100::MouseProtocolEncoding::Sgr,
            7
        ),
        b"\x1b[<0;8;4M"
    );
    assert!(
        mouse_bytes(
            event,
            vt100::MouseProtocolMode::None,
            vt100::MouseProtocolEncoding::Sgr,
            7
        )
        .is_empty()
    );
}

#[test]
fn cursor_queries_use_logical_position_even_when_split_across_chunks() {
    let mut parser = vt100::Parser::new_with_callbacks(24, 80, 0, Protocol::default());
    for byte in "שלום\x1b[6n\x1b[5n\x1b[18t".as_bytes() {
        parser.process(&[*byte]);
    }
    assert_eq!(parser.callbacks().replies, b"\x1b[1;5R\x1b[0n\x1b[8;24;80t");
    assert_eq!(parser.screen().contents(), "שלום");
}

#[test]
fn handles_capability_focus_paste_and_synchronized_output_queries() {
    let mut parser = vt100::Parser::new_with_callbacks(24, 80, 0, Protocol::default());
    parser.process(b"\x1b[c\x1b[>c\x1b[?u\x1b[?1004h\x1b[?2026h\x1b[?2004h\x1b[?2004$p");
    assert!(parser.callbacks().focus_events);
    assert!(parser.callbacks().synchronized_output);
    assert!(parser.screen().bracketed_paste());
    assert_eq!(
        parser.callbacks().replies,
        b"\x1b[?1;2c\x1b[>0;100;0c\x1b[?0u\x1b[?2004;1$y"
    );
    parser.process(b"\x1b[?2026l\x1b[?1004l");
    assert!(!parser.callbacks().focus_events);
    assert!(!parser.callbacks().synchronized_output);
}

#[test]
fn escape_sequences_never_leak_as_visible_text() {
    let mut parser = vt100::Parser::new_with_callbacks(4, 30, 0, Protocol::default());
    parser.process(
        b"\x1b]0;title\x07\x1b]11;?\x1b\\\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\",
    );
    assert_eq!(parser.screen().contents(), "link");
    assert_eq!(
        parser.callbacks().replies,
        b"\x1b]11;rgb:1111/1111/1111\x1b\\"
    );
}

#[test]
fn history_reset_clears_saved_rows_without_erasing_the_draft_or_repeating_rows() {
    let mut parser = vt100::Parser::new_with_callbacks(3, 30, 2, Protocol::default());
    parser.process(b"OLD_0\r\nOLD_1\r\nOLD_2\r\nOLD_3\r\nDRAFT");
    let total = parser.screen().scrollback_total();
    assert_eq!(total, 2);
    assert_eq!(parser.screen().history_since(0).count(), 2);
    assert_eq!(parser.screen().history_since(total).count(), 0);
    let visible = parser.screen().contents();
    parser.process(b"\x1b[3J");
    assert!(parser.callbacks().clear_scrollback);
    assert_eq!(parser.screen().history_since(0).count(), 0);
    assert_eq!(parser.screen().contents(), visible);
    parser.process(b"\r\nNEXT");
    assert_eq!(parser.screen().history_since(total).count(), 1);
}
