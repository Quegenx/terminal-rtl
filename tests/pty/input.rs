use super::harness::PtyHarness;
use portable_pty::PtySize;
use std::{thread, time::Duration};

#[test]
fn real_pty_shift_enter_preserves_modifier_and_restores_host_keyboard() {
    let mut harness = PtyHarness::new("shift-enter");
    harness.until("SHIFT_ENTER_READY");
    assert!(String::from_utf8_lossy(&harness.output).contains("\x1b[>1u"));
    #[cfg(unix)]
    harness.send(b"\x1b[13;2u");
    // ConPTY requests win32-input-mode; native hosts send Shift in its state bits.
    #[cfg(windows)]
    harness.send(b"\x1b[13;28;13;1;16;1_");
    harness.until("NEWLINE_RECEIVED");
    harness.send(b"\r");
    assert_eq!(harness.finish(), 0);
    assert!(String::from_utf8_lossy(&harness.output).contains("\x1b[<1u"));
}

#[test]
fn real_pty_backspace_and_forward_delete_remain_distinct() {
    let mut harness = PtyHarness::new("delete");
    harness.until("DELETE_READY");
    harness.send(b"\x7f\x1b[3~");
    assert_eq!(harness.finish(), 0);
    assert!(String::from_utf8_lossy(&harness.output).contains("DELETE_OK"));
}

#[test]
fn real_pty_queries_paste_resize_and_ctrl_c() {
    for scenario in ["interactive", "interactive-label"] {
        let mut harness = PtyHarness::new(scenario);
        harness.until("QUERY_OK");
        harness.send("\x1b[200~שלום English 123\x1b[201~".as_bytes());
        harness.until("RESIZE_READY");
        harness
            .master
            .resize(PtySize {
                rows: 14,
                cols: 70,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        harness.host.screen_mut().set_size(14, 70);
        harness.send(b"s");
        harness.until("RESIZE_OK");
        harness.send(&[3]);
        assert_eq!(harness.finish(), 7);
        assert!(String::from_utf8_lossy(&harness.output).contains("CTRL_C_OK"));
    }
}

#[test]
fn real_pty_toggle_and_literal_prefix() {
    let mut harness = PtyHarness::new("toggle");
    harness.until("TOGGLE_READY");
    assert!(String::from_utf8_lossy(&harness.output).contains("םולש"));
    harness.output.clear();
    harness.send(b"\x1dr");
    harness.until("שלום");
    harness.send(b"\x1d\x1d");
    harness.until("PREFIX_OK");
    harness.send(b"\x1dx");
    let code = harness.finish();
    assert_eq!(code, 0, "{:?}", String::from_utf8_lossy(&harness.output));
    assert!(String::from_utf8_lossy(&harness.output).contains("PREFIX_OK"));
}

#[test]
fn actual_paste_parser_respects_host_framing_across_chunks() {
    let mut harness = PtyHarness::new("paste-boundary");
    harness.until("PASTE_BOUNDARY_READY");
    for bytes in [
        b"\x1b[20".as_slice(),
        b"0~safe\x1b[200~inside\x1b[2",
        b"01~AFTER",
    ] {
        harness.send(bytes);
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(harness.finish(), 0);
}
