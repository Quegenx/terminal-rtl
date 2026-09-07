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
fn modified_enter_is_distinct_from_submit() {
    assert_eq!(
        key_bytes(KeyEvent::new(KeyCode::Enter, Mod::NONE), false),
        b"\r"
    );
    assert_eq!(
        key_bytes(KeyEvent::new(KeyCode::Enter, Mod::SHIFT), false),
        b"\x1b[13;2u"
    );
    assert_eq!(
        key_bytes(KeyEvent::new(KeyCode::Enter, Mod::ALT), false),
        b"\x1b\r"
    );
    assert_eq!(
        key_bytes(KeyEvent::new(KeyCode::Enter, Mod::SHIFT | Mod::ALT), false),
        b"\x1b[13;4u"
    );
}

#[test]
fn links_follow_cells_through_wrap_scroll_erase_and_alternate_screens() {
    let mut parser = vt100::Parser::new_with_callbacks(2, 8, 10, Protocol::default());
    parser.process(b"\x1b]8;;https://example.com/full-destination\x07ABCDEFGH1234\x1b]8;;\x07!");
    let first = parser
        .screen()
        .cell(0, 0)
        .unwrap()
        .hyperlink()
        .unwrap()
        .clone();
    assert_eq!(
        parser.screen().cell(1, 0).unwrap().hyperlink(),
        Some(&first)
    );
    assert!(parser.screen().cell(1, 4).unwrap().hyperlink().is_none());
    parser.process(b"\r\nnext");
    let saved = parser.screen().history_since(0).next().unwrap();
    assert_eq!(saved[0].hyperlink(), Some(&first));
    parser.process(b"\x1b[H\x1b[K");
    assert!(parser.screen().cell(0, 0).unwrap().hyperlink().is_none());
    parser.process("\x1b]8;id=wide;file:///tmp/example.txt\x1b\\界\x1b]8;;\x1b\\".as_bytes());
    assert_eq!(
        parser.screen().cell(0, 0).unwrap().hyperlink(),
        parser.screen().cell(0, 1).unwrap().hyperlink()
    );
    parser.process(b"\x1b[?1049hALT\x1b[?1049l");
    assert_eq!(
        parser
            .screen()
            .cell(0, 0)
            .unwrap()
            .hyperlink()
            .unwrap()
            .uri(),
        "file:///tmp/example.txt"
    );
    parser.screen_mut().set_size(3, 12);
    assert_eq!(
        parser
            .screen()
            .cell(0, 0)
            .unwrap()
            .hyperlink()
            .unwrap()
            .uri(),
        "file:///tmp/example.txt"
    );
}

#[test]
fn invalid_link_metadata_cannot_emit_terminal_controls() {
    assert!(vt100::Hyperlink::new("https://example.com/\u{009c}".into(), "id".into()).is_none());
    assert!(vt100::Hyperlink::new("https://example.com/\x1b[2J".into(), "id".into()).is_none());
    assert!(vt100::Hyperlink::new("https://example.com".into(), "id;other".into()).is_none());
    assert!(vt100::Hyperlink::new("x".repeat(8193), "id".into()).is_none());
}

#[test]
fn paste_uses_the_childs_mode_and_preserves_original_order() {
    for (text, bracketed) in [
        ("שלום\nworld", "\x1b[200~שלום\nworld\x1b[201~"),
        (
            "שלום\n\x1b[200~literal\x1b[201~",
            "\x1b[200~שלום\n\x1b[200~literal\x1b[201~\x1b[201~",
        ),
    ] {
        assert_eq!(paste_bytes(text, false), text.as_bytes());
        assert_eq!(paste_bytes(text, true), bracketed.as_bytes());
    }
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

#[test]
fn function_editing_and_legacy_mouse_encodings() {
    for (key, expected) in [
        (KeyCode::Home, "\x1b[H"),
        (KeyCode::End, "\x1b[F"),
        (KeyCode::Insert, "\x1b[2~"),
        (KeyCode::Delete, "\x1b[3~"),
        (KeyCode::PageUp, "\x1b[5~"),
        (KeyCode::PageDown, "\x1b[6~"),
    ] {
        assert_eq!(
            key_bytes(KeyEvent::new(key, Mod::NONE), false),
            expected.as_bytes()
        );
    }
    for (number, expected) in [
        (1, "\x1bOP"),
        (2, "\x1bOQ"),
        (3, "\x1bOR"),
        (4, "\x1bOS"),
        (5, "\x1b[15~"),
        (6, "\x1b[17~"),
        (7, "\x1b[18~"),
        (8, "\x1b[19~"),
        (9, "\x1b[20~"),
        (10, "\x1b[21~"),
        (11, "\x1b[23~"),
        (12, "\x1b[24~"),
    ] {
        assert_eq!(
            key_bytes(KeyEvent::new(KeyCode::F(number), Mod::NONE), false),
            expected.as_bytes()
        );
    }
    let event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 3,
        modifiers: Mod::NONE,
    };
    assert_eq!(
        mouse_bytes(
            event,
            vt100::MouseProtocolMode::Press,
            vt100::MouseProtocolEncoding::Default,
            7
        ),
        b"\x1b[M ($"
    );
    assert_eq!(
        mouse_bytes(
            event,
            vt100::MouseProtocolMode::Press,
            vt100::MouseProtocolEncoding::Utf8,
            223
        ),
        "\x1b[M Ā$".as_bytes()
    );
}
